//! Bounded, local PNG/JPEG decoding and session-targeted import transactions.
use crate::{app::Parfait, document_session::SessionKey};
use anyhow::{Context, ensure};
use eframe::egui;
use image::{ImageDecoder, ImageFormat, ImageReader};
use parfait_core::{Document, EditStateId, LayerId};
use std::{
    io::{Cursor, Read},
    path::{Path, PathBuf},
    sync::mpsc,
};

const MAX_FILE_BYTES: u64 = 32 * 1024 * 1024;
const IMPORT_RESERVATION: u64 = 320 * 1024 * 1024;

#[derive(Clone, Copy)]
enum Target {
    New,
    Layer { key: SessionKey, state: EditStateId, layer: LayerId },
}
struct DecodedImage {
    name: String,
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}
struct ImportJob {
    target: Target,
    receiver: mpsc::Receiver<Result<DecodedImage, String>>,
    cancelled: bool,
}
#[derive(Default)]
pub(crate) struct ImageImport {
    job: Option<ImportJob>,
    applying: bool,
}
impl ImageImport {
    pub fn reserved_bytes(&self) -> u64 {
        if self.job.is_some() || self.applying { IMPORT_RESERVATION } else { 0 }
    }
    pub fn dialog_open(&self) -> bool {
        self.job.as_ref().is_some_and(|j| !j.cancelled)
    }
}
fn decode(path: &Path) -> anyhow::Result<DecodedImage> {
    let file =
        std::fs::File::open(path).context("이미지 파일을 열 수 없습니다.")?;
    let metadata = file.metadata()?;
    ensure!(metadata.is_file(), "일반 이미지 파일만 가져올 수 있습니다.");
    ensure!(
        metadata.len() <= MAX_FILE_BYTES,
        "이미지 파일은 32 MiB 이하여야 합니다."
    );
    let mut encoded = Vec::new();
    file.take(MAX_FILE_BYTES + 1).read_to_end(&mut encoded)?;
    ensure!(
        encoded.len() as u64 <= MAX_FILE_BYTES,
        "이미지 파일은 32 MiB 이하여야 합니다."
    );
    let mut reader =
        ImageReader::new(Cursor::new(&encoded)).with_guessed_format()?;
    ensure!(
        matches!(reader.format(), Some(ImageFormat::Png | ImageFormat::Jpeg)),
        "PNG 또는 JPEG 이미지만 가져올 수 있습니다."
    );
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(4096);
    limits.max_image_height = Some(4096);
    limits.max_alloc = Some(128 * 1024 * 1024);
    reader.limits(limits);
    let mut decoder = reader
        .into_decoder()
        .context("이미지 크기 또는 형식을 지원하지 않습니다.")?;
    let (width, height) = decoder.dimensions();
    ensure!(
        width > 0 && height > 0 && width <= 4096 && height <= 4096,
        "이미지의 가로·세로는 1~4096 픽셀이어야 합니다."
    );
    ensure!(
        decoder.total_bytes() <= 128 * 1024 * 1024,
        "이미지 디코딩 메모리 한도를 초과합니다."
    );
    let orientation = decoder.orientation()?;
    let mut image = image::DynamicImage::from_decoder(decoder)?;
    drop(encoded);
    image.apply_orientation(orientation);
    let image = image.into_rgba8();
    let name = path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .chars()
        .take(128)
        .collect();
    Ok(DecodedImage {
        name,
        width: image.width(),
        height: image.height(),
        pixels: image.into_raw(),
    })
}
impl Parfait {
    pub(crate) fn import_image(&mut self, as_layer: bool) {
        if self.dialog_open() || self.image_import.job.is_some() {
            return;
        }
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("PNG / JPEG", &["png", "jpg", "jpeg"])
            .pick_file()
            && let Err(error) = self.queue_image_import(path, as_layer)
        {
            self.error = Some(format!("이미지 가져오기 실패: {error:#}"));
        }
    }
    pub(crate) fn image_import_reason(
        &self,
        as_layer: bool,
    ) -> Option<&'static str> {
        if !self.tabs.enabled {
            return Some("여러 그림을 여는 작업 공간에서 사용할 수 있습니다.");
        }
        if self.dialog_open() || self.image_import.job.is_some() {
            return Some("현재 작업을 마쳐 주세요.");
        }
        if as_layer {
            let Some(key) = self.documents.active_key() else {
                return Some("열린 그림이 없습니다.");
            };
            let doc = &self.documents.get_by_key(key).unwrap().document;
            if let Some(reason) = doc.layer_insertion_reason() {
                return Some(reason);
            }
            return parfait_storage::layer_growth_error(doc, 512);
        }
        if self.documents.tab_order.len() + self.tabs.pending.len()
            >= self.documents.max_tabs
        {
            return Some("열 수 있는 그림 수를 초과합니다.");
        }
        None
    }
    fn queue_image_import(
        &mut self,
        path: PathBuf,
        as_layer: bool,
    ) -> anyhow::Result<()> {
        if let Some(reason) = self.image_import_reason(as_layer) {
            anyhow::bail!(reason);
        }
        self.finish_stroke();
        self.finish_layer_edit();
        let target = if as_layer {
            let key =
                self.documents.active_key().context("열린 그림이 없습니다.")?;
            let session = self.documents.get_by_key(key).unwrap();
            Target::Layer {
                key,
                state: session.history.state_id(),
                layer: session.document.layers[session.document.active].id,
            }
        } else {
            Target::New
        };
        ensure!(
            self.can_reserve_pixel_headroom(IMPORT_RESERVATION),
            "이미지를 가져올 메모리 여유가 없습니다."
        );
        let (sender, receiver) = mpsc::sync_channel(1);
        std::thread::Builder::new().name("parfait-image-import".into()).spawn(
            move || {
                let result = std::panic::catch_unwind(|| decode(&path))
                    .map_err(|_| {
                        "이미지 디코더가 예기치 않게 종료되었습니다.".into()
                    })
                    .and_then(|result| result.map_err(|e| format!("{e:#}")));
                let _ = sender.send(result);
            },
        )?;
        self.image_import.job =
            Some(ImportJob { target, receiver, cancelled: false });
        Ok(())
    }
    pub(crate) fn poll_image_import(&mut self) {
        let Some(job) = &self.image_import.job else {
            return;
        };
        let result = match job.receiver.try_recv() {
            Ok(result) => result,
            Err(mpsc::TryRecvError::Empty) => return,
            Err(mpsc::TryRecvError::Disconnected) => {
                Err("이미지 가져오기 작업이 중단되었습니다.".into())
            }
        };
        let job = self.image_import.job.take().unwrap();
        if job.cancelled {
            return;
        }
        self.image_import.applying = true;
        let result = result
            .map_err(anyhow::Error::msg)
            .and_then(|image| self.apply_import(job.target, image));
        self.image_import.applying = false;
        if let Err(error) = result {
            self.error = Some(format!("이미지 가져오기 실패: {error:#}"));
        }
    }
    fn apply_import(
        &mut self,
        target: Target,
        image: DecodedImage,
    ) -> anyhow::Result<()> {
        ensure!(!self.dialog_open(), "현재 편집 창을 마쳐 주세요.");
        match target {
            Target::New => {
                ensure!(
                    self.documents.logical_bytes().saturating_add(
                        u64::from(image.width) * u64::from(image.height) * 4
                    ) <= self.documents.logical_limit,
                    "열린 그림의 픽셀 메모리 한도를 초과합니다."
                );
                let doc = Document::from_rgba_image(
                    image.name,
                    image.width,
                    image.height,
                    image.pixels,
                )
                .context("이미지 픽셀 자료가 올바르지 않습니다.")?;
                self.new_document_tab(doc)?;
            }
            Target::Layer { key, state, layer } => {
                let session = self
                    .documents
                    .get_by_key(key)
                    .context("가져올 대상 그림이 닫혔습니다.")?;
                ensure!(
                    session.history.state_id() == state,
                    "가져오는 동안 대상 그림이 변경되었습니다."
                );
                let doc = &session.document;
                let index = doc
                    .layer_index(layer)
                    .context("가져올 대상 레이어가 없습니다.")?;
                ensure!(
                    image.width <= doc.width && image.height <= doc.height,
                    "이미지가 캔버스보다 큽니다. 새 그림으로 가져오거나 캔버스 크기를 늘려 주세요."
                );
                ensure!(
                    self.documents.logical_bytes().saturating_add(
                        u64::from(doc.width) * u64::from(doc.height) * 4
                    ) <= self.documents.logical_limit,
                    "열린 그림의 픽셀 메모리 한도를 초과합니다."
                );
                let mut candidate = doc.clone();
                candidate.active = index;
                if let Some(reason) =
                    candidate.layer_insertion_reason().or_else(|| {
                        parfait_storage::layer_growth_error(
                            &candidate,
                            image.name.len(),
                        )
                    })
                {
                    anyhow::bail!(reason);
                }
                ensure!(
                    candidate.import_raster_layer(
                        image.name,
                        image.width,
                        image.height,
                        &image.pixels
                    ),
                    "레이어를 추가할 수 없습니다."
                );
                self.edit_named_for(key, "Import image layer", |doc| {
                    *doc = candidate
                });
                let session = self.documents.get_by_key_mut(key).unwrap();
                ensure!(
                    session.history.state_id() != state,
                    "이미지 가져오기를 확정할 수 없습니다."
                );
                session.editing.mask_target = None;
                session.reset_layer_selection_view();
                session.reveal_layer(session.document.active);
                self.force_save_for(key);
            }
        }
        Ok(())
    }
    pub(crate) fn image_import_ui(&mut self, ctx: &egui::Context) {
        if self.image_import.job.is_none() {
            return;
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(50));
        if !self.image_import.dialog_open() {
            return;
        }
        let response =
            egui::Modal::new(egui::Id::new("image_import")).show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label("이미지 가져오는 중");
                });
                ui.button("취소").clicked()
            });
        if response.inner
            || (response.is_top_modal
                && ctx.input_mut(|i| {
                    i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)
                }))
        {
            self.image_import.job.as_mut().unwrap().cancelled = true;
        }
    }
}

#[cfg(test)]
mod tests;

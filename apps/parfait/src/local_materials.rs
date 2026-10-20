use crate::app::Parfait;
use serde::{Deserialize, Serialize};
use std::{
    io::Write,
    path::{Path, PathBuf},
};

#[derive(Clone, Copy, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub(crate) enum MaterialKind {
    Image,
    Template,
}
#[derive(Serialize, Deserialize)]
struct MaterialInfo {
    version: u32,
    name: String,
    kind: MaterialKind,
}

pub(crate) fn read_local_image(
    path: &Path,
) -> anyhow::Result<parfait_core::PixelClipboard> {
    let mut reader = image::ImageReader::open(path)?.with_guessed_format()?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(4096);
    limits.max_image_height = Some(4096);
    limits.max_alloc = Some(64 * 1024 * 1024);
    reader.limits(limits);
    let image = reader.decode()?.into_rgba8();
    let clip = parfait_core::PixelClipboard {
        width: image.width(),
        height: image.height(),
        origin: [0; 2],
        pixels: image.into_raw(),
    };
    anyhow::ensure!(
        clip.validate(),
        "이미지는 1~4096픽셀 크기의 RGBA 데이터여야 합니다."
    );
    Ok(clip)
}
impl Parfait {
    pub(crate) fn use_local_material(
        &mut self,
        kind: MaterialKind,
        path: &Path,
    ) -> anyhow::Result<()> {
        if kind == MaterialKind::Template {
            anyhow::ensure!(
                self.tabs.enabled,
                "템플릿 사용에는 다중 문서 작업 화면이 필요합니다."
            );
            anyhow::ensure!(
                self.reserve_pixel_headroom(512 * 1024 * 1024),
                "템플릿을 읽을 메모리가 부족합니다."
            );
            let document = parfait_storage::Store::open_read_only(path)?
                .load()?
                .ok_or_else(|| anyhow::anyhow!("템플릿에 그림이 없습니다."))?
                .document;
            self.new_document_tab_as(document, false)?;
        } else {
            if let Some(reason) = self
                .command_disabled_reason(crate::shortcuts::Command::EditPaste)
            {
                anyhow::bail!(reason);
            }
            anyhow::ensure!(
                self.reserve_pixel_headroom(128 * 1024 * 1024),
                "소재 이미지를 읽을 메모리가 부족합니다."
            );
            let clip = read_local_image(path)?;
            let document = &self.documents.active_session.document;
            let origin = [
                (document.width as i32 - clip.width as i32) / 2,
                (document.height as i32 - clip.height as i32) / 2,
            ];
            self.edit_named("이미지 소재 사용", |doc| {
                doc.paste_pixels(&clip, origin);
            });
            let active = self.documents.active_session.document.active;
            self.set_layer_selection(1 << active, active);
            self.reveal_layer(active);
        }
        Ok(())
    }
    pub(crate) fn register_local_material(
        &mut self,
        name: &str,
        kind: MaterialKind,
    ) -> anyhow::Result<PathBuf> {
        anyhow::ensure!(
            !name.trim().is_empty()
                && name.chars().count() <= 128
                && !name.chars().any(char::is_control),
            "이름은 1~128자이며 제어 문자를 포함할 수 없습니다."
        );
        let root = self.recovery_path.parent().unwrap().join("materials");
        std::fs::create_dir_all(&root)?;
        anyhow::ensure!(
            std::fs::read_dir(&root)?.take(257).count() < 256,
            "로컬 소재 목록은 256개까지 등록할 수 있습니다."
        );
        let mut bytes = [0; 16];
        getrandom::fill(&mut bytes).map_err(|e| {
            anyhow::anyhow!("소재 식별자를 만들지 못했습니다: {e}")
        })?;
        let id = bytes.iter().map(|v| format!("{v:02x}")).collect::<String>();
        let folder = root.join(id);
        std::fs::create_dir(&folder)?;
        let mut doc = self.documents.active_session.document.clone();
        let path = match kind {
            MaterialKind::Image => {
                let rect =
                    doc.selection_bounds().unwrap_or(parfait_core::PixelRect {
                        x0: 0,
                        y0: 0,
                        x1: doc.width,
                        y1: doc.height,
                    });
                let composite = doc.composite();
                let mut pixels = Vec::with_capacity(
                    ((rect.x1 - rect.x0) * (rect.y1 - rect.y0) * 4) as usize,
                );
                for y in rect.y0..rect.y1 {
                    for x in rect.x0..rect.x1 {
                        let i = (y * doc.width + x) as usize;
                        let mut pixel: [u8; 4] =
                            composite[i * 4..i * 4 + 4].try_into().unwrap();
                        pixel[3] = ((u16::from(pixel[3])
                            * u16::from(doc.selection_value(i))
                            + 127)
                            / 255) as u8;
                        pixels.extend_from_slice(&pixel);
                    }
                }
                let path = folder.join("image.png");
                image::save_buffer_with_format(
                    &path,
                    &pixels,
                    rect.x1 - rect.x0,
                    rect.y1 - rect.y0,
                    image::ColorType::Rgba8,
                    image::ImageFormat::Png,
                )?;
                path
            }
            MaterialKind::Template => {
                doc.name = name.into();
                let path = folder.join("template.parfait");
                Self::write_project_document(&doc, &path)?;
                path
            }
        };
        let info = MaterialInfo { version: 1, name: name.trim().into(), kind };
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(folder.join("material.json"))?;
        file.write_all(&serde_json::to_vec(&info)?)?;
        file.sync_all()?;
        Ok(path)
    }
    pub(crate) fn local_materials(
        &self,
    ) -> Vec<(String, MaterialKind, PathBuf)> {
        let root = self.recovery_path.parent().unwrap().join("materials");
        let Ok(entries) = std::fs::read_dir(&root) else {
            return Vec::new();
        };
        let mut items = Vec::new();
        for entry in entries.take(256).flatten() {
            let path = entry.path();
            let name = entry.file_name();
            let Some(id) = name.to_str() else {
                continue;
            };
            if id.len() != 32
                || !id.bytes().all(|b| b.is_ascii_hexdigit())
                || entry
                    .file_type()
                    .is_ok_and(|t| !t.is_dir() || t.is_symlink())
            {
                continue;
            }
            let meta = path.join("material.json");
            if !std::fs::metadata(&meta).is_ok_and(|m| m.len() <= 4096) {
                continue;
            }
            let Ok(bytes) = std::fs::read(meta) else {
                continue;
            };
            let Ok(info) = serde_json::from_slice::<MaterialInfo>(&bytes)
            else {
                continue;
            };
            if info.version != 1
                || info.name.chars().count() > 128
                || info.name.chars().any(char::is_control)
            {
                continue;
            }
            let file = path.join(if info.kind == MaterialKind::Image {
                "image.png"
            } else {
                "template.parfait"
            });
            if file
                .symlink_metadata()
                .is_ok_and(|m| m.is_file() && !m.file_type().is_symlink())
            {
                items.push((info.name, info.kind, file));
            }
        }
        items.sort_by(|a, b| a.0.cmp(&b.0));
        items
    }
}

//! One in-flight full preview. A shared document snapshot never blocks pointer UI.
use crate::{app::Parfait, document_session::SessionKey};
use eframe::egui;
use parfait_core::{Document, LayerId};
use std::sync::mpsc::{self, Receiver, TryRecvError};
pub(crate) struct CompositeJob {
    pub receiver: Receiver<CompositeResult>,
    pub snapshot: std::sync::Arc<Document>,
    pub reserved_bytes: u64,
}
pub(crate) struct CompositeResult {
    owner: SessionKey,
    revision: u64,
    overlay: Option<LayerId>,
    image: egui::ColorImage,
}
fn composite(
    document: &Document,
    overlay: Option<LayerId>,
) -> egui::ColorImage {
    let mut pixels = document.composite();
    for (i, p) in pixels.as_chunks_mut::<4>().0.iter_mut().enumerate() {
        if p[3] == 255 {
            continue;
        }
        let x = i % document.width as usize;
        let y = i / document.width as usize;
        let bg = if (x / 16 + y / 16).is_multiple_of(2) { 235. } else { 218. };
        let alpha = f32::from(p[3]) / 255.;
        for c in &mut p[..3] {
            *c = (f32::from(*c) * alpha + bg * (1. - alpha)).round() as u8;
        }
        p[3] = 255;
    }
    if let Some(index) = overlay.and_then(|id| document.layer_index(id))
        && let Some(mask) = &document.layers[index].mask
    {
        crate::mask_area::tint_mask_area(
            &mut pixels,
            [document.width, document.height],
            &mask.coverage,
            parfait_core::PixelRect {
                x0: 0,
                y0: 0,
                x1: document.width,
                y1: document.height,
            },
        );
    }
    egui::ColorImage::from_rgba_unmultiplied(
        [document.width as usize, document.height as usize],
        &pixels,
    )
}
impl Parfait {
    pub(crate) fn poll_composite(&mut self, ctx: &egui::Context) {
        let result = match self
            .render_cache
            .composite_job
            .as_ref()
            .map(|job| job.receiver.try_recv())
        {
            Some(Ok(result)) => result,
            Some(Err(TryRecvError::Disconnected)) => {
                self.render_cache.composite_job = None;
                return;
            }
            _ => return,
        };
        self.render_cache.composite_job = None;
        if result.owner != self.documents.active_session.key
            || result.image.size
                != [
                    self.documents.active_session.document.width as usize,
                    self.documents.active_session.document.height as usize,
                ]
            || result.overlay != self.active_mask_overlay()
            || (result.revision <= self.render_cache.rendered
                && self.render_cache.texture.is_some())
        {
            return;
        }
        if let Some(texture) = &mut self.render_cache.texture {
            texture.set(result.image, egui::TextureOptions::LINEAR);
        } else {
            self.render_cache.texture = Some(ctx.load_texture(
                "drawing",
                result.image,
                egui::TextureOptions::LINEAR,
            ));
        }
        self.render_cache.rendered = result.revision;
        self.render_cache.rendered_overlay = result.overlay;
    }
    pub(crate) fn request_composite(&mut self, ctx: &egui::Context) -> bool {
        if self.render_cache.composite_job.is_some() {
            return true;
        }
        let document =
            std::sync::Arc::new(self.documents.active_session.document.clone());
        let owner = self.documents.active_session.key;
        let overlay = self.active_mask_overlay();
        let ctx = ctx.clone();
        let (sender, receiver) = mpsc::sync_channel(1);
        self.render_cache.composite_job = Some(CompositeJob {
            receiver,
            snapshot: document.clone(),
            reserved_bytes: u64::from(document.width)
                * u64::from(document.height)
                * 8
                + 1024 * 1024,
        });
        // Only one snapshot/result is retained; later slider changes coalesce.
        let result = std::thread::Builder::new()
            .name("canvas-composite".into())
            .spawn(move || {
                let image = composite(&document, overlay);
                if sender
                    .send(CompositeResult {
                        owner,
                        revision: document.revision,
                        overlay,
                        image,
                    })
                    .is_ok()
                {
                    ctx.request_repaint();
                }
            });
        if result.is_err() {
            self.render_cache.composite_job = None;
            false
        } else {
            true
        }
    }
}

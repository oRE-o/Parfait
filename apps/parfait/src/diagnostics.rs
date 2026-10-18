//! Opt-in framebuffer export for visual QA, independent of desktop capture tools.
use eframe::egui;
use std::path::PathBuf;

pub struct Capture {
    path: PathBuf,
    frames: u8,
}
impl Capture {
    pub fn new(path: PathBuf) -> Self {
        Self { path, frames: 0 }
    }
    pub fn update(&mut self, ctx: &egui::Context) -> anyhow::Result<()> {
        if self.frames < 5 {
            self.frames += 1;
            if self.frames == 5 {
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
            }
        }
        let captured = ctx.input(|i| {
            i.events.iter().find_map(|event| {
                if let egui::Event::Screenshot { image, .. } = event {
                    Some(image.clone())
                } else {
                    None
                }
            })
        });
        if let Some(image) = captured {
            let pixels: Vec<u8> = image
                .pixels
                .iter()
                .flat_map(|p| p.to_srgba_unmultiplied())
                .collect();
            image::save_buffer_with_format(
                &self.path,
                &pixels,
                image.size[0] as u32,
                image.size[1] as u32,
                image::ColorType::Rgba8,
                image::ImageFormat::Png,
            )?;
        }
        Ok(())
    }
}

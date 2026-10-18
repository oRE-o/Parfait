use crate::canvas::View;
use eframe::egui;
use parfait_core::{Brush, Document, History, Stroke};
use parfait_storage::{Autosave, SaveEvent, Store, lock_session};
use std::{
    fs::File,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

pub struct Parfait {
    pub capture: Option<crate::diagnostics::Capture>,
    pub doc: Document,
    pub brush: Brush,
    pub history: History,
    pub stroke: Option<Stroke>,
    pub view: View,
    pub texture: Option<egui::TextureHandle>,
    pub rendered: u64,
    pub saved: u64,
    pub submitted: u64,
    pub autosave: Autosave,
    pub last_submit: Instant,
    pub error: Option<String>,
    pub notice: Option<String>,
    pub project: Option<PathBuf>,
    pub recovery_path: PathBuf,
    pub show_help: bool,
    pub confirm_new: bool,
    pub tip_name: String,
    pub hide_panels: bool,
    pub pressure: f32,
    pub saving_failed: bool,
    close_pending: bool,
    _lock: File,
}
impl Parfait {
    pub fn open(dir: PathBuf) -> anyhow::Result<Self> {
        let lock = lock_session(&dir.join("session.lock"))?;
        let path = dir.join("recovery.parfait");
        let store = Store::open(&path)?;
        let unclean = store.unclean()?;
        let loaded = store.load()?;
        let fallback = loaded.as_ref().is_some_and(|l| l.fell_back);
        let existed = loaded.is_some();
        let doc = loaded
            .map(|l| l.document)
            .unwrap_or_else(|| Document::new(1024, 768));
        drop(store);
        let autosave = Autosave::start(path.clone())?;
        let notice = if fallback {
            Some("Recovered an earlier valid save. The newest snapshot was damaged.".into())
        } else if existed && unclean {
            Some("Recovered your drawing after an unexpected exit.".into())
        } else if existed {
            Some("Welcome back. Your previous drawing is restored.".into())
        } else {
            None
        };
        autosave.submit(doc.clone());
        let submitted = doc.revision;
        Ok(Self {
            capture: None,
            doc,
            brush: Brush::default(),
            history: History::default(),
            stroke: None,
            view: View::default(),
            texture: None,
            rendered: 0,
            saved: 0,
            submitted,
            autosave,
            last_submit: Instant::now(),
            error: None,
            notice,
            project: None,
            recovery_path: path,
            show_help: false,
            confirm_new: false,
            tip_name: "Round pen".into(),
            hide_panels: false,
            pressure: 1.,
            saving_failed: false,
            close_pending: false,
            _lock: lock,
        })
    }
    pub fn edit(&mut self, action: impl FnOnce(&mut Document)) {
        self.history.checkpoint(&self.doc);
        action(&mut self.doc);
        self.doc.touch();
    }
    pub fn undo(&mut self) {
        if self.stroke.is_none() {
            self.history.undo(&mut self.doc);
        }
    }
    pub fn redo(&mut self) {
        if self.stroke.is_none() {
            self.history.redo(&mut self.doc);
        }
    }
    pub fn force_save(&mut self) {
        self.autosave.submit(self.doc.clone());
        self.submitted = self.doc.revision;
        self.last_submit = Instant::now();
    }
    pub fn save_project(&mut self, save_as: bool) {
        let path = if !save_as { self.project.clone() } else { None }.or_else(|| {
            rfd::FileDialog::new()
                .add_filter("Parfait project", &["parfait"])
                .set_file_name(format!("{}.parfait", self.doc.name))
                .save_file()
        });
        if let Some(path) = path {
            if let Some(name) = path.file_stem().and_then(|s| s.to_str()) {
                self.doc.name = name.to_owned();
                self.doc.touch();
            }
            match self.write_project(&path) {
                Ok(()) => {
                    self.project = Some(path);
                    self.force_save();
                    self.notice = Some("Project saved.".into());
                }
                Err(e) => self.error = Some(format!("Save failed: {e:#}")),
            }
        }
    }
    fn write_project(&self, path: &Path) -> anyhow::Result<()> {
        let mut store = Store::open(path)?;
        let mut doc = self.doc.clone();
        if let Some(old) = store.load()? {
            doc.revision = doc.revision.max(old.document.revision) + 1;
        }
        store.save(&doc)?;
        store.mark_clean()?;
        store.checkpoint()?;
        Ok(())
    }
    pub fn open_project(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Parfait project", &["parfait"])
            .pick_file()
        {
            let load = || -> anyhow::Result<Document> {
                // Backup the current drawing before replacing the working document.
                self.write_project(&self.recovery_path.with_file_name("before-open.parfait"))?;
                Store::open(&path)?
                    .load()?
                    .map(|l| l.document)
                    .ok_or_else(|| anyhow::anyhow!("Project contains no snapshots"))
            };
            match load() {
                Ok(mut doc) => {
                    doc.revision = self.doc.revision + 1;
                    self.doc = doc;
                    self.history = History::default();
                    self.project = Some(path);
                    self.view = View::default();
                    self.force_save();
                }
                Err(e) => self.error = Some(format!("Open failed: {e:#}")),
            }
        }
    }
    pub fn new_document(&mut self) {
        match self.write_project(&self.recovery_path.with_file_name("before-new.parfait")) {
            Ok(()) => {
                let rev = self.doc.revision + 1;
                self.doc = Document::new(1024, 768);
                self.doc.revision = rev;
                self.history = History::default();
                self.project = None;
                self.view = View::default();
                self.force_save();
            }
            Err(e) => self.error = Some(format!("Could not back up the current drawing: {e:#}")),
        }
    }
    pub fn export(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("PNG image", &["png"])
            .set_file_name(format!("{}.png", self.doc.name))
            .save_file()
        {
            match image::save_buffer_with_format(
                path,
                &self.doc.composite(),
                self.doc.width,
                self.doc.height,
                image::ColorType::Rgba8,
                image::ImageFormat::Png,
            ) {
                Ok(()) => self.notice = Some("PNG exported.".into()),
                Err(e) => self.error = Some(e.to_string()),
            }
        }
    }
    pub fn import_tip(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Brush tip", &["png"])
            .pick_file()
        {
            match image::ImageReader::open(&path)
                .map_err(anyhow::Error::from)
                .and_then(|r| r.decode().map_err(Into::into))
            {
                Ok(img) => {
                    let img = img.thumbnail(256, 256).to_rgba8();
                    let (w, h) = img.dimensions();
                    let alpha = img.pixels().any(|p| p[3] < 255);
                    let mask = img
                        .pixels()
                        .map(|p| {
                            if alpha {
                                p[3]
                            } else {
                                255 - ((p[0] as u32 + p[1] as u32 + p[2] as u32) / 3) as u8
                            }
                        })
                        .collect();
                    self.brush.tip = parfait_core::BrushTip::Image {
                        width: w,
                        height: h,
                        mask: std::sync::Arc::new(mask),
                    };
                    self.tip_name = path
                        .file_stem()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned();
                    self.notice=Some("Brush tip imported. Transparent tips use alpha; opaque tips use inverted luminance.".into());
                }
                Err(e) => self.error = Some(format!("Brush import failed: {e:#}")),
            }
        }
    }
    fn saving(&mut self) {
        while let Some(event) = self.autosave.poll() {
            match event {
                SaveEvent::Saved(rev) => {
                    self.saved = self.saved.max(rev);
                    self.saving_failed = false;
                }
                SaveEvent::Failed(e) => {
                    self.error = Some(format!(
                        "Automatic save failed. Your drawing is still in memory. {e}"
                    ));
                    self.saving_failed = true;
                    self.close_pending = false;
                    self.submitted = 0;
                    self.last_submit = Instant::now();
                }
            }
        }
        let interval = if self.saving_failed {
            Duration::from_secs(3)
        } else {
            Duration::from_millis(400)
        };
        if self.doc.revision != self.submitted && self.last_submit.elapsed() > interval {
            self.force_save();
        }
    }
}
impl eframe::App for Parfait {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.saving();
        self.shortcuts(&ctx);
        self.top_panel(ui);
        self.bottom_panel(ui);
        if !self.hide_panels {
            self.tool_panel(ui);
            self.layer_panel(ui);
        }
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(egui::Color32::from_rgb(23, 24, 29)))
            .show(ui, |ui| self.canvas(ui));
        if self.show_help {
            egui::Window::new("Keyboard shortcuts")
                .open(&mut self.show_help)
                .resizable(false)
                .show(&ctx, |ui| {
                    for line in crate::shortcuts::HELP {
                        ui.label(*line);
                    }
                });
        }
        if self.confirm_new {
            egui::Window::new("New drawing")
                .collapsible(false)
                .resizable(false)
                .show(&ctx, |ui| {
                    ui.label(
                        "Start a fresh 1024 × 768 drawing? Your current drawing will be backed up.",
                    );
                    ui.horizontal(|ui| {
                        if ui.button("Create").clicked() {
                            self.new_document();
                            self.confirm_new = false;
                        }
                        if ui.button("Cancel").clicked() {
                            self.confirm_new = false;
                        }
                    });
                });
        }
        if let Some(error) = self.error.clone() {
            egui::Window::new("Attention")
                .collapsible(false)
                .show(&ctx, |ui| {
                    ui.label(error);
                    if ui.button("OK").clicked() {
                        self.error = None;
                    }
                });
        }
        ctx.request_repaint_after(Duration::from_millis(100));
        if let Some(capture) = &mut self.capture
            && let Err(error) = capture.update(&ctx)
        {
            self.error = Some(format!("Screenshot failed: {error}"));
        }
        if ctx.input(|i| i.viewport().close_requested()) && self.saved < self.doc.revision {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.stroke = None;
            self.close_pending = true;
            self.force_save();
        }
        if self.close_pending && self.saved >= self.doc.revision && !self.saving_failed {
            self.close_pending = false;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.autosave.finish(self.doc.clone());
    }
}

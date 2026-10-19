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
    pub calibration: parfait_core::PressureCalibration,
    pub calibration_dialog: Option<crate::calibration::CalibrationDialog>,
    pub workspace: crate::workspace::Workspace,
    pub presets: Vec<crate::brushes::Preset>,
    pub brush_library: crate::brushes::BrushLibrary,
    pub preferences_saved: Option<crate::preferences::Preferences>,
    pub preferences_tick: Instant,
    pub preferences_read_failed: bool,
    pub pressure_seen: bool,
    pub pressure_tab: bool,
    pub brush_details: crate::brush_details::BrushDetails,
    pub capture: Option<crate::diagnostics::Capture>,
    pub navigation_tool: u8,
    pub layer_previews: Vec<egui::TextureHandle>,
    pub(crate) mask_previews: Vec<Option<egui::TextureHandle>>,
    pub(crate) mask_target: Option<parfait_core::LayerId>,
    pub preview_revision: u64,
    pub doc: Document,
    pub brush: Brush,
    pub history: History,
    pub stroke: Option<Stroke>,
    pub(crate) layer_edit: Option<crate::editing::LayerEdit>,
    pub(crate) cancelled_layer_drag: bool,
    pub(crate) layer_list: crate::layers::LayerList,
    pub(crate) rename_layer: Option<crate::layer_organize::RenameLayer>,
    pub view: View,
    pub display: crate::display::DisplaySettings,
    pub show_display_settings: bool,
    pub keymap: crate::shortcuts::Keymap,
    pub shortcut_editor: Option<crate::shortcuts::ShortcutEditor>,
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
    pub show_licenses: bool,
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
        let mut app = Self {
            calibration: Default::default(),
            calibration_dialog: None,
            workspace: Default::default(),
            presets: crate::brushes::defaults(),
            brush_library: Default::default(),
            preferences_saved: None,
            preferences_tick: Instant::now(),
            preferences_read_failed: false,
            pressure_seen: false,
            pressure_tab: false,
            brush_details: Default::default(),
            capture: None,
            navigation_tool: 0,
            layer_previews: Vec::new(),
            mask_previews: Vec::new(),
            mask_target: None,
            preview_revision: u64::MAX,
            doc,
            brush: Brush::default(),
            history: History::default(),
            stroke: None,
            layer_edit: None,
            cancelled_layer_drag: false,
            layer_list: Default::default(),
            rename_layer: None,
            view: View::default(),
            display: Default::default(),
            show_display_settings: false,
            keymap: Default::default(),
            shortcut_editor: None,
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
            show_licenses: false,
            confirm_new: false,
            tip_name: "Round pen".into(),
            hide_panels: false,
            pressure: 1.,
            saving_failed: false,
            close_pending: false,
            _lock: lock,
        };
        app.restore_preferences();
        Ok(app)
    }
    pub fn dialog_open(&self) -> bool {
        self.rename_layer.is_some()
            || self.calibration_dialog.is_some()
            || self.brush_details.dialog_open()
            || self.shortcut_editor.is_some()
            || self.show_display_settings
            || self.show_help
            || self.show_licenses
            || self.confirm_new
            || self.error.is_some()
    }
    #[cfg(test)]
    pub fn edit(&mut self, action: impl FnOnce(&mut Document)) {
        self.edit_named("Edit", action);
    }
    pub(crate) fn edit_named(&mut self, label: &str, action: impl FnOnce(&mut Document)) {
        self.finish_stroke();
        self.finish_layer_edit();
        self.history.begin(&self.doc, label);
        let revision = self.doc.revision;
        action(&mut self.doc);
        if self.history.commit(&self.doc) && self.doc.revision == revision {
            self.doc.touch();
        }
    }
    pub fn undo(&mut self) {
        if self.stroke.is_none() {
            self.layer_list = Default::default();
            self.finish_layer_edit();
            self.history.undo(&mut self.doc);
        }
    }
    pub fn redo(&mut self) {
        if self.stroke.is_none() {
            self.layer_list = Default::default();
            self.finish_layer_edit();
            self.history.redo(&mut self.doc);
        }
    }
    pub fn force_save(&mut self) {
        self.autosave.submit(self.doc.clone());
        self.submitted = self.doc.revision;
        self.last_submit = Instant::now();
    }
    fn maintenance_interval(&self) -> Duration {
        if self.stroke.is_some()
            || self.close_pending
            || self.saved < self.submitted
            || self.saved < self.doc.revision
        {
            Duration::from_millis(100)
        } else {
            Duration::from_secs(1)
        }
    }
    pub fn request_close(&mut self, ctx: &egui::Context) {
        if self.saved < self.doc.revision || self.saving_failed {
            self.finish_stroke();
            self.finish_layer_edit();
            self.close_pending = true;
            self.force_save();
        } else {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
    pub fn save_project(&mut self, save_as: bool) {
        self.finish_layer_edit();
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
        self.finish_layer_edit();
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
                    self.mask_target = None;
                    self.history = History::default();
                    self.layer_list = Default::default();
                    self.project = Some(path);
                    self.view = View::default();
                    self.force_save();
                }
                Err(e) => self.error = Some(format!("Open failed: {e:#}")),
            }
        }
    }
    pub fn new_document(&mut self) {
        self.finish_layer_edit();
        match self.write_project(&self.recovery_path.with_file_name("before-new.parfait")) {
            Ok(()) => {
                let rev = self.doc.revision + 1;
                self.doc = Document::new(1024, 768);
                self.mask_target = None;
                self.doc.revision = rev;
                self.history = History::default();
                self.layer_list = Default::default();
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
        self.prepare_layer_list(&ctx);
        self.shortcuts(&ctx);
        // Native window controls remain usable while an editor dialog is open.
        self.title_bar(ui);
        ui.add_enabled_ui(!self.dialog_open(), |ui| {
            self.top_panel(ui);
            self.bottom_panel(ui);
            self.workspace_ui(ui);
            self.settle_layer_edit(&ctx);
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE.fill(egui::Color32::from_gray(self.display.background)))
                .show(ui, |ui| self.canvas(ui));
        });
        self.brush_details_ui(&ctx);
        self.calibration_ui(&ctx);
        self.display_settings_ui(&ctx);
        self.shortcut_settings_ui(&ctx);
        self.layer_rename_ui(&ctx);
        if self.show_help {
            let mut help_open = true;
            egui::Window::new("Keyboard shortcuts")
                .open(&mut help_open)
                .resizable(false)
                .show(&ctx, |ui| {
                    egui::ScrollArea::vertical()
                        .max_height((ctx.content_rect().height() - 130.).max(100.))
                        .show(ui, |ui| {
                            for line in crate::shortcuts::HELP {
                                ui.label(*line);
                            }
                            ui.separator();
                            for &command in crate::shortcuts::Command::ALL {
                                let keys = self.keymap.hint(command);
                                if !keys.is_empty() {
                                    ui.horizontal(|ui| {
                                        ui.add_sized(
                                            [210., 20.],
                                            egui::Label::new(command.label()),
                                        );
                                        ui.label(keys);
                                    });
                                }
                            }
                        });
                    ui.separator();
                    crate::widgets::action_row(ui, |ui| {
                        if crate::widgets::action_button(ui, "Close", 64.).clicked() {
                            self.show_help = false;
                        }
                        if crate::widgets::action_button(ui, "Shortcut settings…", 140.).clicked()
                        {
                            self.open_shortcut_settings();
                        }
                    });
                });
            self.show_help &= help_open;
        }
        if self.confirm_new {
            egui::Window::new("New drawing")
                .collapsible(false)
                .resizable(false)
                .show(&ctx, |ui| {
                    ui.label(
                        "Start a fresh 1024 × 768 drawing? Your current drawing will be backed up.",
                    );
                    ui.separator();
                    crate::widgets::action_row(ui, |ui| {
                        if crate::widgets::action_button(ui, "Create", 72.).clicked() {
                            self.new_document();
                            self.confirm_new = false;
                        }
                        if crate::widgets::action_button(ui, "Cancel", 72.).clicked() {
                            self.confirm_new = false;
                        }
                    });
                });
        }
        if self.show_licenses {
            let mut licenses_open = true;
            egui::Window::new("Third-party notices")
                .open(&mut licenses_open)
                .default_width(600.)
                .show(&ctx, |ui| {
                    ui.label("Bundled fonts · SIL Open Font License 1.1");
                    egui::ScrollArea::vertical()
                        .max_height((ctx.content_rect().height() - 180.).clamp(100., 450.))
                        .show(ui, |ui| {
                            ui.label("Pretendard · Kil Hyung-jin · SIL Open Font License 1.1");
                            ui.hyperlink_to(
                                "Pretendard upstream source",
                                "https://github.com/orioncactus/pretendard",
                            );
                            ui.label(include_str!("../assets/pretendard/LICENSE.txt"));
                            ui.separator();
                            ui.label("Font Awesome Free 6.7.2 · Fonticons, Inc.");
                            ui.hyperlink_to(
                                "Font Awesome upstream source",
                                "https://github.com/FortAwesome/Font-Awesome/tree/6.7.2",
                            );
                            ui.label(include_str!("../assets/fontawesome/LICENSE.txt"));
                        });
                    ui.separator();
                    crate::widgets::action_row(ui, |ui| {
                        if crate::widgets::action_button(ui, "Close", 64.).clicked() {
                            self.show_licenses = false;
                        }
                    });
                });
            self.show_licenses &= licenses_open;
        }
        if let Some(error) = self.error.clone() {
            egui::Window::new("Attention")
                .collapsible(false)
                .show(&ctx, |ui| {
                    ui.label(error);
                    ui.separator();
                    crate::widgets::action_row(ui, |ui| {
                        if crate::widgets::action_button(ui, "OK", 64.).clicked() {
                            self.error = None;
                        }
                    });
                });
        }
        ctx.request_repaint_after(self.maintenance_interval());
        crate::chrome::resize_edges(ui);
        self.persist_preferences(false);
        if let Some(capture) = &mut self.capture
            && let Err(error) = capture.update(&ctx)
        {
            self.error = Some(format!("Screenshot failed: {error}"));
        }
        if ctx.input(|i| i.viewport().close_requested()) && self.saved < self.doc.revision {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.finish_stroke();
            self.finish_layer_edit();
            self.close_pending = true;
            self.force_save();
        }
        if self.close_pending && self.saved >= self.doc.revision && !self.saving_failed {
            self.close_pending = false;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.persist_preferences(true);
        self.autosave.finish(self.doc.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layer_limit_commands_do_not_edit_revision_or_consume_undo() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = Parfait::open(dir.path().to_owned()).unwrap();
        app.doc = Document::new(4, 4);
        while app.doc.layers.len() < 64 {
            app.doc.add_layer();
        }
        app.edit(|document| document.layers[1].opacity = 0.5);
        let before = app.doc.clone();
        let ctx = egui::Context::default();
        app.execute_command(crate::shortcuts::Command::NewLayer, &ctx);
        app.duplicate_layer();
        assert_eq!(app.doc, before);
        app.undo();
        assert_eq!(app.doc.layers[1].opacity, 1.);
    }

    #[test]
    fn idle_maintenance_slows_down_without_delaying_pending_saves_or_strokes() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = Parfait::open(dir.path().to_path_buf()).unwrap();
        app.doc = Document::new(16, 16);
        app.saved = app.doc.revision;
        app.submitted = app.saved;
        assert_eq!(app.maintenance_interval(), Duration::from_secs(1));
        app.doc.touch();
        assert_eq!(app.maintenance_interval(), Duration::from_millis(100));
        app.saved = app.doc.revision;
        app.submitted = app.saved + 1;
        assert_eq!(app.maintenance_interval(), Duration::from_millis(100));
        app.submitted = app.saved;
        app.close_pending = true;
        assert_eq!(app.maintenance_interval(), Duration::from_millis(100));
        app.close_pending = false;
        app.stroke = Some(Stroke::begin(
            &mut app.doc,
            Brush::default(),
            parfait_core::Point {
                x: 4.,
                y: 4.,
                pressure: 1.,
            },
        ));
        app.saved = app.doc.revision;
        assert_eq!(app.maintenance_interval(), Duration::from_millis(100));
    }
}

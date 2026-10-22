use crate::{
    document_session::SessionKey, documents::DocumentManager,
    window_render_cache::WindowRenderCache,
};
use eframe::egui;
#[cfg(test)]
use parfait_core::Stroke;
use parfait_core::{Brush, Document};
use parfait_storage::{Autosave, SaveEvent, Store, lock_session};
use std::{
    fs::File,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

pub struct Parfait {
    pub(crate) documents: DocumentManager,
    pub(crate) source_saves: crate::source_saves::SourceSaves,
    pub(crate) recovery_runtime:
        Option<crate::recovery_runtime::RecoveryRuntime>,
    pub(crate) options: crate::settings::Options,
    pub(crate) settings: crate::settings::SettingsState,
    pub(crate) screen_picker: Option<crate::screen_picker::ScreenPicker>,
    pub(crate) screen_capture_busy:
        std::sync::Arc<std::sync::atomic::AtomicBool>,
    pub(crate) native_dialog: Option<crate::native_content::NativeDialog>,
    pub(crate) image_edit: crate::image_edit::ImageEditState,
    pub(crate) tabs: crate::document_tabs::DocumentTabs,
    pub(crate) render_cache: WindowRenderCache,
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
    pub tools: crate::tools::ToolSettings,
    pub brush: Brush,
    pub display: crate::display::DisplaySettings,
    pub show_display_settings: bool,
    pub keymap: crate::shortcuts::Keymap,
    pub shortcut_editor: Option<crate::shortcuts::ShortcutEditor>,
    pub autosave: Autosave,
    pub error: Option<String>,
    pub notice: Option<String>,
    pub recovery_path: PathBuf,
    pub show_help: bool,
    pub show_licenses: bool,
    pub confirm_new: bool,
    pub tip_name: String,
    pub hide_panels: bool,
    pub pressure: f32,
    close_pending: bool,
    _lock: File,
}
impl Parfait {
    pub fn open(dir: PathBuf) -> anyhow::Result<Self> {
        Self::open_mode(dir, true)
    }
    #[cfg(test)]
    pub fn open_legacy(dir: PathBuf) -> anyhow::Result<Self> {
        Self::open_mode(dir, false)
    }
    #[cfg(test)]
    pub fn open_document_tabs(dir: PathBuf) -> anyhow::Result<Self> {
        Self::open_mode(dir, true)
    }
    fn open_mode(dir: PathBuf, multi: bool) -> anyhow::Result<Self> {
        let lock = lock_session(&dir.join("session.lock"))?;
        let path = dir.join("recovery.parfait");
        let (doc, unclean, fallback, existed, autosave) = if multi {
            (Document::new(1, 1), false, false, false, Autosave::disabled())
        } else {
            let store = Store::open(&path)?;
            let unclean = store.unclean()?;
            let loaded = store.load()?;
            let fallback = loaded.as_ref().is_some_and(|l| l.fell_back);
            let existed = loaded.is_some();
            let doc = loaded
                .map(|l| l.document)
                .unwrap_or_else(|| Document::new(1024, 768));
            drop(store);
            (doc, unclean, fallback, existed, Autosave::start(path.clone())?)
        };
        let notice = if fallback {
            Some("Recovered an earlier valid save. The newest snapshot was damaged.".into())
        } else if existed && unclean {
            Some("Recovered your drawing after an unexpected exit.".into())
        } else if existed {
            Some("Welcome back. Your previous drawing is restored.".into())
        } else {
            None
        };
        if !multi {
            autosave.submit(doc.clone());
        }
        let submitted = doc.revision;
        let mut documents = DocumentManager::new(doc)?;
        documents.active_session.recovery.submitted = submitted;
        documents.active_session.untouched_new = !existed;
        let mut app = Self {
            documents,
            source_saves: Default::default(),
            recovery_runtime: None,
            options: Default::default(),
            settings: Default::default(),
            screen_picker: None,
            screen_capture_busy: Default::default(),
            native_dialog: None,
            image_edit: Default::default(),
            tabs: Default::default(),
            render_cache: WindowRenderCache::default(),
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
            tools: Default::default(),
            brush: Brush::default(),
            display: Default::default(),
            show_display_settings: false,
            keymap: Default::default(),
            shortcut_editor: None,
            autosave,
            error: None,
            notice,
            recovery_path: path,
            show_help: false,
            show_licenses: false,
            confirm_new: false,
            tip_name: "Round pen".into(),
            hide_panels: false,
            pressure: 1.,
            close_pending: false,
            _lock: lock,
        };
        app.restore_preferences();
        if multi {
            app.tabs.loader =
                Some(crate::project_loader::ProjectLoader::new()?);
            app.start_document_recovery()?;
            app.tabs.enabled = true;
        }
        Ok(app)
    }
    pub fn dialog_open(&self) -> bool {
        let document_dialog = self
            .documents
            .active_key()
            .and_then(|key| self.documents.get_by_key(key))
            .is_some_and(|session| {
                session.layer_ui.rename.is_some()
                    || session.layer_ui.details.is_some()
            });
        self.tab_close_dialog_open()
            || self.settings.dialog.is_some()
            || self.settings.quick_open
            || self.settings.about_open
            || self.screen_picker.is_some()
            || self.native_dialog.is_some()
            || self.image_edit.dialog.is_some()
            || document_dialog
            || !self.source_saves.prompts.is_empty()
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
    pub(crate) fn edit_named(
        &mut self,
        label: &str,
        action: impl FnOnce(&mut Document),
    ) {
        let Some(target) = self.documents.active_key() else {
            return;
        };
        self.edit_named_for(target, label, action);
    }
    pub(crate) fn edit_named_for(
        &mut self,
        target: SessionKey,
        label: &str,
        action: impl FnOnce(&mut Document),
    ) {
        self.finish_stroke_for(target);
        self.finish_layer_edit_for(target);
        if !self.begin_history_for(target, label) {
            return;
        }
        let logical = self.documents.logical_bytes();
        let limit = self.documents.logical_limit;
        let session = self.documents.get_by_key_mut(target).unwrap();
        let before_bytes = session.document.logical_pixel_bytes();
        let revision = session.document.revision;
        action(&mut session.document);
        session.document.prune_layer_colors();
        if self.tabs.enabled
            && logical - before_bytes + session.document.logical_pixel_bytes()
                > limit
        {
            session.cancel_pending();
            self.notice = Some(
                "Edit exceeds the open-document logical pixel budget.".into(),
            );
            return;
        }
        let changed = session.commit_edit();
        if changed && session.document.revision == revision {
            session.document.touch();
        }
        let initial_recovery = changed && !session.recovery.registered;
        if initial_recovery && self.recovery_runtime.is_some() {
            self.force_save_for(target);
        }
    }
    #[cfg(test)]
    pub(crate) fn begin_history(&mut self, label: &str) -> bool {
        let Some(target) = self.documents.active_key() else {
            return false;
        };
        self.begin_history_for(target, label)
    }
    pub(crate) fn begin_history_for(
        &mut self,
        target: SessionKey,
        label: &str,
    ) -> bool {
        if !self.preflight_edit(target, label) {
            return false;
        }
        let session = self.documents.get_by_key_mut(target).unwrap();
        match session.history.try_begin(&session.document, label) {
            Ok(()) => true,
            Err(error) => {
                session.edit_reservation = 0;
                self.error = Some(format!("Could not start edit: {error}"));
                false
            }
        }
    }
    pub fn undo(&mut self) {
        let Some(target) = self.documents.active_key() else {
            return;
        };
        let session = self.documents.get_by_key_mut(target).unwrap();
        if session.editing.stroke.is_some() {
            return;
        }
        session.reset_layer_selection_view();
        self.finish_layer_edit_for(target);
        if self.navigate_history(target, |history, document| {
            history.undo(document)
        }) {
            let session = self.documents.get_by_key_mut(target).unwrap();
            session.reveal_layer(session.document.active);
        }
    }
    pub fn redo(&mut self) {
        let Some(target) = self.documents.active_key() else {
            return;
        };
        let session = self.documents.get_by_key_mut(target).unwrap();
        if session.editing.stroke.is_some() {
            return;
        }
        session.reset_layer_selection_view();
        self.finish_layer_edit_for(target);
        if self.navigate_history(target, |history, document| {
            history.redo(document)
        }) {
            let session = self.documents.get_by_key_mut(target).unwrap();
            session.reveal_layer(session.document.active);
        }
    }
    pub fn force_save(&mut self) {
        let Some(target) = self.documents.active_key() else {
            return;
        };
        self.force_save_for(target);
    }
    pub(crate) fn force_save_for(&mut self, target: SessionKey) {
        if self.recovery_runtime.is_some() {
            self.submit_document_recovery(target);
            return;
        }
        if self.tabs.enabled {
            return;
        }
        let Some(session) = self.documents.get_by_key_mut(target) else {
            return;
        };
        self.autosave.submit(session.document.clone());
        session.recovery.submitted = session.document.revision;
        session.recovery.last_submit = Instant::now();
    }
    fn maintenance_interval(&self) -> Duration {
        if self.tabs.enabled {
            let busy = !self.tabs.pending.is_empty()
                || self
                    .recovery_runtime
                    .as_ref()
                    .is_some_and(|runtime| runtime.worker.pending())
                || self.documents.sessions().any(|s| {
                    !s.pristine()
                        && (s.recovery.failed
                            || s.document.revision != s.recovery.submitted)
                });
            return Duration::from_millis(if busy { 100 } else { 1000 });
        }
        let session = &*self.documents.active_session;
        if session.editing.stroke.is_some()
            || self.close_pending
            || session.recovery.saved < session.recovery.submitted
            || session.recovery.saved < session.document.revision
        {
            Duration::from_millis(100)
        } else {
            Duration::from_secs(1)
        }
    }
    pub fn request_close(&mut self, ctx: &egui::Context) {
        if self.recovery_runtime.is_some() {
            match self.flush_document_recovery(true) {
                Ok(()) => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
                Err(error) => {
                    self.notice = Some(format!("Exit cancelled: {error:#}"));
                    ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                }
            }
            return;
        }
        if self.tabs.enabled {
            if !self.documents.is_empty() {
                self.finish_stroke();
                self.finish_layer_edit();
            }
            if !self.tabs.pending.is_empty()
                || self
                    .documents
                    .sessions()
                    .any(|s| s.modified() && !s.pristine())
            {
                self.notice = Some("Save all drawings and finish/cancel pending opens before exiting.".into());
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            } else {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            return;
        }
        if self.documents.active_session.recovery.saved
            < self.documents.active_session.document.revision
            || self.documents.active_session.recovery.failed
        {
            self.finish_stroke();
            self.finish_layer_edit();
            self.close_pending = true;
            self.force_save();
        } else {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
    pub fn save_project(&mut self, save_as: bool) {
        let Some(target) = self.documents.active_key() else {
            return;
        };
        self.save_project_for(target, save_as);
    }
    pub(crate) fn save_project_for(
        &mut self,
        target: SessionKey,
        save_as: bool,
    ) {
        self.finish_layer_edit_for(target);
        let session = self.documents.get_by_key(target).unwrap();
        let path = if !save_as { session.project.clone() } else { None }
            .or_else(|| {
                rfd::FileDialog::new()
                    .add_filter("Parfait project", &["parfait"])
                    .set_file_name(format!("{}.parfait", session.document.name))
                    .save_file()
            });
        if let Some(path) = path {
            match self.save_project_to_for(target, path) {
                Ok(()) => {
                    self.notice = Some(
                        if self.recovery_runtime.is_some() {
                            "Saving original file…"
                        } else {
                            "Project saved."
                        }
                        .into(),
                    )
                }
                Err(error) => {
                    self.error = Some(format!("Save failed: {error:#}"))
                }
            }
        }
    }
    fn write_project(&self, path: &Path) -> anyhow::Result<()> {
        Self::write_project_document(
            &self.documents.active_session.document,
            path,
        )
    }
    pub fn open_project(&mut self) {
        if !self.tabs.enabled && !self.documents.is_empty() {
            self.finish_layer_edit();
        }
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Parfait project", &["parfait"])
            .pick_file()
        {
            match self.open_project_from(path) {
                Ok(()) => {}
                Err(e) => self.error = Some(format!("Open failed: {e:#}")),
            }
        }
    }
    pub fn new_document(&mut self) {
        if self.tabs.enabled {
            const NEW_PIXELS: u64 = 1024 * 768 * 8;
            if self.documents.tab_order.len() + self.tabs.pending.len()
                >= self.documents.max_tabs
                || self.documents.logical_bytes().saturating_add(NEW_PIXELS)
                    > self.documents.logical_limit
                || !self.reserve_pixel_headroom(NEW_PIXELS)
            {
                self.notice = Some(
                    "Not enough tab or pixel budget for a new drawing.".into(),
                );
                return;
            }
            match self.new_document_tab_as(Document::new(1024, 768), true) {
                Ok(id) => {
                    self.documents.get_mut(id).unwrap().untouched_new = true
                }
                Err(error) => self.error = Some(error.to_string()),
            }
            return;
        }
        self.finish_layer_edit();
        match self.write_project(
            &self.recovery_path.with_file_name("before-new.parfait"),
        ) {
            Ok(()) => {
                let mut document = Document::new(1024, 768);
                document.revision =
                    self.documents.active_session.document.revision + 1;
                match crate::document_session::DocumentSession::new(document) {
                    Ok(session) => self.documents.replace_single(session),
                    Err(error) => {
                        self.error = Some(format!(
                            "Could not create drawing: {error:#}"
                        ));
                        return;
                    }
                }
                self.force_save();
            }
            Err(e) => {
                self.error = Some(format!(
                    "Could not back up the current drawing: {e:#}"
                ))
            }
        }
    }
    pub fn export(&mut self) {
        if self.documents.is_empty() {
            return;
        }
        let session = &*self.documents.active_session;
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("PNG image", &["png"])
            .set_file_name(format!("{}.png", session.document.name))
            .save_file()
        {
            match image::save_buffer_with_format(
                path,
                &session.document.composite_for_export(),
                session.document.width,
                session.document.height,
                image::ColorType::Rgba8,
                image::ImageFormat::Png,
            ) {
                Ok(()) => self.notice = Some("PNG exported.".into()),
                Err(e) => self.error = Some(e.to_string()),
            }
        }
    }
    fn saving(&mut self) {
        if self.recovery_runtime.is_some() {
            self.maintain_document_recovery();
            return;
        }
        if self.tabs.enabled {
            self.poll_project_opens();
            return;
        }
        let Some(target) = self.documents.active_key() else {
            return;
        };
        let session = self.documents.get_by_key_mut(target).unwrap();
        while let Some(event) = self.autosave.poll() {
            match event {
                SaveEvent::Saved(rev) => {
                    session.recovery.saved = session.recovery.saved.max(rev);
                    session.recovery.failed = false;
                }
                SaveEvent::Failed(e) => {
                    self.error = Some(format!(
                        "Automatic save failed. Your drawing is still in memory. {e}"
                    ));
                    session.recovery.failed = true;
                    self.close_pending = false;
                    session.recovery.submitted = 0;
                    session.recovery.last_submit = Instant::now();
                }
            }
        }
        let interval = if session.recovery.failed {
            Duration::from_secs(3)
        } else {
            Duration::from_millis(u64::from(self.options.autosave_delay_ms))
        };
        if session.document.revision != session.recovery.submitted
            && session.recovery.last_submit.elapsed() > interval
        {
            self.force_save_for(target);
        }
    }
}
impl Parfait {
    pub(crate) fn draw_ui(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        self.saving();
        self.poll_transform_preview(&ctx);
        self.poll_local_preview(&ctx);
        self.poll_controllers(&ctx);
        if !self.documents.is_empty() {
            self.prepare_layer_list(&ctx);
        }
        self.shortcuts(&ctx);
        // Native window controls remain usable while an editor dialog is open.
        self.menu_bar(ui);
        ui.add_enabled_ui(!self.dialog_open(), |ui| {
            if self.documents.is_empty() {
                if self.tabs.enabled {
                    self.document_tabs_ui(ui);
                }
                egui::CentralPanel::default().show(ui, |ui| {
                    ui.heading("No drawing open");
                    if ui.button("New drawing").clicked() {
                        self.new_document();
                    }
                    if ui.button("Open project").clicked() {
                        self.open_project();
                    }
                    if let Some(message) = &self.notice {
                        ui.label(message);
                    }
                });
                return;
            }
            self.workspace_ui(ui);
            self.settle_layer_edit(&ctx);
            egui::CentralPanel::default()
                .frame(
                    egui::Frame::NONE.fill(egui::Color32::from_gray(
                        self.display.background,
                    )),
                )
                .show(ui, |ui| {
                    self.top_panel(ui);
                    self.bottom_panel(ui);
                    self.canvas_document_strip(ui);
                    self.canvas(ui);
                });
        });
        self.settings_ui(&ctx);
        self.screen_picker_ui(&ctx);
        self.image_edit_dialog_ui(&ctx);
        self.native_content_ui(&ctx);
        self.source_save_dialogs(&ctx);
        self.tab_close_prompt_ui(&ctx);
        self.brush_details_ui(&ctx);
        self.calibration_ui(&ctx);
        self.display_settings_ui(&ctx);
        self.shortcut_settings_ui(&ctx);
        if !self.documents.is_empty() {
            self.layer_rename_ui(&ctx);
            self.layer_details_ui(&ctx);
        }
        if self.show_help {
            let mut help_open = true;
            egui::Window::new("Keyboard shortcuts")
                .open(&mut help_open)
                .resizable(false)
                .show(&ctx, |ui| {
                    egui::ScrollArea::vertical()
                        .max_height(
                            (ctx.content_rect().height() - 130.).max(100.),
                        )
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
                        if crate::widgets::action_button(ui, "Close", 64.)
                            .clicked()
                        {
                            self.show_help = false;
                        }
                        if crate::widgets::action_button(
                            ui,
                            "Shortcut settings…",
                            140.,
                        )
                        .clicked()
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
            egui::Window::new("Attention").collapsible(false).show(
                &ctx,
                |ui| {
                    ui.label(error);
                    ui.separator();
                    crate::widgets::action_row(ui, |ui| {
                        if crate::widgets::action_button(ui, "OK", 64.)
                            .clicked()
                        {
                            self.error = None;
                        }
                    });
                },
            );
        }
        ctx.request_repaint_after(self.maintenance_interval());
        crate::chrome::resize_edges(ui);
        if !ctx.input(|i| i.pointer.any_down()) {
            self.persist_preferences(false);
        }
        if !self.image_edit_preview_pending()
            && let Some(capture) = &mut self.capture
            && let Err(error) = capture.update(&ctx)
        {
            self.error = Some(format!("Screenshot failed: {error}"));
        }
        if self.tabs.enabled {
            if ctx.input(|i| i.viewport().close_requested()) {
                self.request_close(&ctx);
            }
            return;
        }
        if ctx.input(|i| i.viewport().close_requested())
            && self.documents.active_session.recovery.saved
                < self.documents.active_session.document.revision
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.finish_stroke();
            self.finish_layer_edit();
            self.close_pending = true;
            self.force_save();
        }
        if self.close_pending
            && self.documents.active_session.recovery.saved
                >= self.documents.active_session.document.revision
            && !self.documents.active_session.recovery.failed
        {
            self.close_pending = false;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}
impl eframe::App for Parfait {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.; 4]
    }
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.draw_ui(ui);
    }
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.persist_preferences(true);
        if self.recovery_runtime.is_some() {
            if !self.recovery_runtime.as_ref().unwrap().shutdown_ready
                && let Err(error) = self.flush_document_recovery(true)
            {
                eprintln!("Final recovery failed: {error:#}");
            }
            self.recovery_runtime.as_mut().unwrap().worker.finish();
        } else if !self.tabs.enabled {
            self.autosave
                .finish(self.documents.active_session.document.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layer_limit_commands_do_not_edit_revision_or_consume_undo() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = Parfait::open_legacy(dir.path().to_owned()).unwrap();
        app.documents.active_session.document = Document::new(4, 4);
        while app.documents.active_session.document.layers.len() < 64 {
            app.documents.active_session.document.add_layer();
        }
        app.edit(|document| document.layers[1].opacity = 0.5);
        let before = app.documents.active_session.document.clone();
        let ctx = egui::Context::default();
        app.execute_command(crate::shortcuts::Command::NewLayer, &ctx);
        app.duplicate_layer();
        assert_eq!(app.documents.active_session.document, before);
        app.undo();
        assert_eq!(app.documents.active_session.document.layers[1].opacity, 1.);
    }

    #[test]
    fn idle_maintenance_slows_down_without_delaying_pending_saves_or_strokes() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = Parfait::open_legacy(dir.path().to_path_buf()).unwrap();
        app.documents.active_session.document = Document::new(16, 16);
        app.documents.active_session.recovery.saved =
            app.documents.active_session.document.revision;
        app.documents.active_session.recovery.submitted =
            app.documents.active_session.recovery.saved;
        assert_eq!(app.maintenance_interval(), Duration::from_secs(1));
        app.documents.active_session.document.touch();
        assert_eq!(app.maintenance_interval(), Duration::from_millis(100));
        app.documents.active_session.recovery.saved =
            app.documents.active_session.document.revision;
        app.documents.active_session.recovery.submitted =
            app.documents.active_session.recovery.saved + 1;
        assert_eq!(app.maintenance_interval(), Duration::from_millis(100));
        app.documents.active_session.recovery.submitted =
            app.documents.active_session.recovery.saved;
        app.close_pending = true;
        assert_eq!(app.maintenance_interval(), Duration::from_millis(100));
        app.close_pending = false;
        app.documents.active_session.editing.stroke = Some(Stroke::begin(
            &mut app.documents.active_session.document,
            Brush::default(),
            parfait_core::Point { x: 4., y: 4., pressure: 1. },
        ));
        app.documents.active_session.recovery.saved =
            app.documents.active_session.document.revision;
        assert_eq!(app.maintenance_interval(), Duration::from_millis(100));
    }
}

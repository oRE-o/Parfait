use super::*;
fn setup() -> (tempfile::TempDir, Parfait, egui::Context) {
    let dir = tempfile::tempdir().unwrap();
    let mut app = Parfait::open_legacy(dir.path().to_owned()).unwrap();
    app.documents.active_session.document = parfait_core::Document::new(4, 4);
    let ctx = egui::Context::default();
    crate::theme::apply(&ctx);
    (dir, app, ctx)
}
#[test]
fn settings_drafts_cancel_without_mutating_preferences_or_artwork() {
    for command in [
        Command::EditPreferences,
        Command::EditModifierSettings,
        Command::EditCommandBarSettings,
        Command::EditQuickSettings,
        Command::EditToolCycleSettings,
        Command::EditFontSettings,
        Command::EditPrivacySettings,
        Command::EditSystemServices,
    ] {
        let (_dir, mut app, ctx) = setup();
        let before = app.options.clone();
        let workspace = app.workspace.clone();
        let calibration = app.calibration.clone();
        let artwork = app.documents.active_session.document.clone();
        app.execute_command(command, &ctx);
        assert!(app.settings.dialog.is_some());
        app.settings.dialog.as_mut().unwrap().candidate.autosave_delay_ms =
            1000;
        if command == Command::EditPreferences {
            let dialog = app.settings.dialog.as_mut().unwrap();
            dialog.workspace.panels[0].inactive_opacity = 0.4;
            dialog.calibration.points.insert(1, [0.4, 0.8]);
        }
        let mut out = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(960., 640.),
                )),
                ..Default::default()
            },
            |ui| app.settings_ui(ui.ctx()),
        );
        out.textures_delta.clear();
        for id in ["settings_accept", "settings_cancel"] {
            let r = ctx
                .data(|d| d.get_temp::<egui::Rect>(egui::Id::new(id)))
                .unwrap();
            assert!(ctx.content_rect().contains_rect(r), "{command:?}: {r:?}");
        }
        let mut out = ctx.run_ui(
            egui::RawInput {
                events: vec![egui::Event::Key {
                    key: egui::Key::Escape,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: Default::default(),
                }],
                ..Default::default()
            },
            |ui| app.settings_ui(ui.ctx()),
        );
        out.textures_delta.clear();
        assert!(app.settings.dialog.is_none());
        assert_eq!(app.options, before);
        assert!(app.workspace.same_preferences(&workspace));
        assert_eq!(app.calibration, calibration);
        assert!(
            app.documents.active_session.document.same_edit_state(&artwork)
        );
    }
}
#[test]
fn options_roundtrip_without_injecting_new_key_bindings_or_starting_devices() {
    let (dir, mut app, _ctx) = setup();
    app.options.autosave_delay_ms = 1000;
    app.options.clipboard_write = false;
    app.options.toolbar = vec![Command::Undo, Command::EditFill];
    app.options.quick = vec![Command::ToolInk, Command::EditInvert];
    let expected = app.options.clone();
    let keymap = app.keymap.clone();
    app.persist_preferences(true);
    assert!(app.error.is_none());
    drop(app);
    let app = Parfait::open_legacy(dir.path().to_owned()).unwrap();
    assert_eq!(app.options, expected);
    assert!(app.keymap == keymap);
    assert!(app.settings.gilrs.is_none());
}
#[test]
fn corrupted_lists_controller_profiles_and_font_parameters_are_rejected() {
    let mut options = Options::default();
    options.cycle.clear();
    assert!(!options.validate());
    let options =
        Options { toolbar: vec![Command::Undo; 17], ..Default::default() };
    assert!(!options.validate());
    let options = Options { font_size: f32::NAN, ..Default::default() };
    assert!(!options.validate());
    let mut options = Options::default();
    options.controllers.registered = vec!["not-a-device".into()];
    assert!(!options.validate());
    let mut options = Options::default();
    let id = "0".repeat(32);
    options.controllers.registered = vec![id.clone()];
    options.controllers.bindings = vec![ControllerBinding {
        device: id,
        button: "South".into(),
        command: Command::Undo,
    }];
    assert!(options.validate());
    options.controllers.bindings[0].command = Command::DeleteLayer;
    assert!(!options.validate());
}
#[test]
fn invalid_custom_font_preserves_current_font_style_and_artwork() {
    let (dir, app, ctx) = setup();
    let path = dir.path().join("bad.ttf");
    std::fs::write(&path, b"not a font").unwrap();
    let mut options = app.options.clone();
    options.font_path = Some(path);
    options.font_size = 20.;
    assert!(apply_font(&ctx, &options).is_err());
    assert_eq!(
        ctx.style_of(egui::Theme::Dark).text_styles[&egui::TextStyle::Body]
            .size,
        12.
    );
}
#[test]
fn tool_cycle_changes_only_tool_state_and_quick_menu_respects_modal_ownership()
{
    let (_dir, mut app, ctx) = setup();
    let before = app.documents.active_session.document.clone();
    app.options.cycle = vec![Command::ToolInk, Command::ToolEraser];
    app.execute_command(Command::ToolInk, &ctx);
    app.execute_command(Command::CycleTools, &ctx);
    assert_eq!(app.active_tool_command(), Some(Command::ToolEraser));
    app.execute_command(Command::ShowQuickMenu, &ctx);
    assert!(app.settings.quick_open);
    assert!(app.dialog_open());
    let mut out = ctx.run_ui(
        egui::RawInput {
            events: vec![egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Default::default(),
            }],
            ..Default::default()
        },
        |ui| app.settings_ui(ui.ctx()),
    );
    out.textures_delta.clear();
    assert!(!app.settings.quick_open);
    assert!(app.documents.active_session.document.same_edit_state(&before));
}

#[test]
fn settings_apply_failure_restores_active_options_fonts_and_retains_retryable_draft()
 {
    fn frame(app: &mut Parfait, ctx: &egui::Context, events: Vec<egui::Event>) {
        let mut out = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(960., 640.),
                )),
                events,
                ..Default::default()
            },
            |ui| app.settings_ui(ui.ctx()),
        );
        out.textures_delta.clear();
    }
    fn accept(app: &mut Parfait, ctx: &egui::Context) {
        frame(app, ctx, vec![]);
        frame(app, ctx, vec![]);
        let point = ctx
            .data(|d| {
                d.get_temp::<egui::Rect>(egui::Id::new("settings_accept"))
            })
            .unwrap()
            .center();
        frame(
            app,
            ctx,
            vec![
                egui::Event::PointerMoved(point),
                egui::Event::PointerButton {
                    pos: point,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: Default::default(),
                },
            ],
        );
        frame(
            app,
            ctx,
            vec![egui::Event::PointerButton {
                pos: point,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            }],
        );
    }
    let (dir, mut app, ctx) = setup();
    app.persist_preferences(true);
    let before = app.options.clone();
    let artwork = app.documents.active_session.document.clone();
    app.execute_command(Command::EditPreferences, &ctx);
    app.settings.dialog.as_mut().unwrap().candidate.font_size = 18.;
    let path = dir.path().join("preferences.sqlite");
    let backup = dir.path().join("preferences.saved");
    std::fs::rename(&path, &backup).unwrap();
    std::fs::create_dir(&path).unwrap();
    accept(&mut app, &ctx);
    assert_eq!(app.options, before);
    assert!(app.settings.dialog.as_ref().unwrap().error.is_some());
    assert_eq!(
        ctx.style_of(egui::Theme::Dark).text_styles[&egui::TextStyle::Body]
            .size,
        12.
    );
    assert!(app.documents.active_session.document.same_edit_state(&artwork));
    std::fs::remove_dir(&path).unwrap();
    std::fs::rename(&backup, &path).unwrap();
    accept(&mut app, &ctx);
    assert!(app.settings.dialog.is_none());
    assert_eq!(app.options.font_size, 18.);
    drop(app);
    let reopened = Parfait::open_legacy(dir.path().to_owned()).unwrap();
    assert_eq!(reopened.options.font_size, 18.);
}

use crate::{app::Parfait, shortcuts::Command};
use eframe::egui;
use parfait_core::{BlendMode, Document};

fn frame(
    app: &mut Parfait,
    context: &egui::Context,
    time: &mut f64,
    events: Vec<egui::Event>,
) {
    *time += 0.03;
    let mut output = context.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(960., 800.),
            )),
            time: Some(*time),
            events,
            ..Default::default()
        },
        |ui| {
            app.prepare_layer_list(ui.ctx());
            app.shortcuts(ui.ctx());
            ui.add_enabled_ui(!app.dialog_open(), |ui| {
                egui::Panel::left("through_layers").exact_size(320.).show(
                    ui,
                    |ui| {
                        app.palette(ui, crate::workspace::Palette::Layers, 700.)
                    },
                );
            });
            app.layer_details_ui(ui.ctx());
            app.settle_layer_edit(ui.ctx());
        },
    );
    output.textures_delta.clear();
}

fn rect(context: &egui::Context, name: &str) -> Option<egui::Rect> {
    context.data(|data| data.get_temp(egui::Id::new(name)))
}

fn click(
    app: &mut Parfait,
    context: &egui::Context,
    time: &mut f64,
    target: egui::Rect,
) {
    for pressed in [true, false] {
        frame(
            app,
            context,
            time,
            vec![
                egui::Event::PointerMoved(target.center()),
                egui::Event::PointerButton {
                    pos: target.center(),
                    pressed,
                    button: egui::PointerButton::Primary,
                    modifiers: Default::default(),
                },
            ],
        );
    }
    frame(app, context, time, vec![]);
}

#[test]
fn actual_blend_popup_is_folder_only_and_shares_draft_cancellation_without_losing_redo()
 {
    for folder in [false, true] {
        let directory = tempfile::tempdir().unwrap();
        let mut app =
            Parfait::open_legacy(directory.path().to_owned()).unwrap();
        app.documents.active_session.document = Document::new(8, 8);
        if folder {
            app.documents
                .active_session
                .document
                .group_layers_in_folder(2, "Pass".into())
                .unwrap();
        }
        let before = app.documents.active_session.document.clone();
        let context = egui::Context::default();
        crate::theme::apply(&context);
        let mut time = 0.;
        for _ in 0..2 {
            frame(&mut app, &context, &mut time, vec![]);
        }
        click(
            &mut app,
            &context,
            &mut time,
            rect(&context, "layer_blend").unwrap(),
        );
        let option = rect(&context, "blend_choice_through");
        if !folder {
            assert!(option.is_none());
            assert!(
                app.documents.active_session.document.same_edit_state(&before)
            );
            continue;
        }
        click(&mut app, &context, &mut time, option.unwrap());
        assert_eq!(
            app.documents.active_session.document.layers[2].blend_mode,
            BlendMode::Through
        );
        assert_eq!(app.documents.active_session.history.rows().count(), 2);
        assert!(app.documents.active_session.document.validate());
        let path = directory.path().join("pass.parfait");
        app.save_project_to(path.clone()).unwrap();
        assert_eq!(
            parfait_storage::Store::open(&path)
                .unwrap()
                .load()
                .unwrap()
                .unwrap()
                .document
                .layers,
            app.documents.active_session.document.layers
        );
        app.undo();
        assert!(app.documents.active_session.document.same_edit_state(&before));
        assert!(app.documents.active_session.history.can_redo());
        app.open_layer_details();
        for _ in 0..2 {
            frame(&mut app, &context, &mut time, vec![]);
        }
        click(
            &mut app,
            &context,
            &mut time,
            rect(&context, "layer_details_blend").unwrap(),
        );
        click(
            &mut app,
            &context,
            &mut time,
            rect(&context, "blend_choice_through").unwrap(),
        );
        assert_eq!(
            app.documents
                .active_session
                .layer_ui
                .details
                .as_ref()
                .unwrap()
                .draft
                .blend_mode,
            BlendMode::Through
        );
        assert!(app.documents.active_session.document.same_edit_state(&before));
        click(
            &mut app,
            &context,
            &mut time,
            rect(&context, "layer_details_cancel").unwrap(),
        );
        assert!(app.documents.active_session.layer_ui.details.is_none());
        assert!(app.documents.active_session.history.can_redo());
        assert!(app.documents.active_session.document.same_edit_state(&before));
    }
}

#[test]
fn through_clipping_guards_keep_suppressed_settings_reversible_and_orphans_unlinkable()
 {
    let directory = tempfile::tempdir().unwrap();
    let mut app = Parfait::open_legacy(directory.path().to_owned()).unwrap();
    app.documents.active_session.document = Document::new(8, 8);
    app.documents.active_session.document.add_layer();
    app.documents
        .active_session
        .document
        .group_layers_in_folder(2, "Pass".into())
        .unwrap();
    app.documents.active_session.document.layers[2].blend_mode =
        BlendMode::Through;
    app.select_layer(3);
    let context = egui::Context::default();
    assert!(app.command_disabled_reason(Command::LayerClipping).is_some());
    let before = app.documents.active_session.document.clone();
    app.execute_command(Command::LayerClipping, &context);
    assert!(app.documents.active_session.document.same_edit_state(&before));
    app.select_layer(2);
    assert!(app.command_disabled_reason(Command::LayerClipping).is_none());
    app.execute_command(Command::LayerClipping, &context);
    assert!(app.documents.active_session.document.layers[2].clipped);
    assert_eq!(
        app.documents.active_session.document.layers[2].blend_mode,
        BlendMode::Through
    );
    assert!(app.documents.active_session.document.validate());
    app.undo();
    assert!(!app.documents.active_session.document.layers[2].clipped);
    assert_eq!(
        app.documents.active_session.document.layers[2].blend_mode,
        BlendMode::Through
    );
    app.select_layer(3);
    app.documents.active_session.document.layers[3].clipped = true;
    app.documents.active_session.document.touch();
    assert!(app.command_disabled_reason(Command::LayerClipping).is_none());
    app.execute_command(Command::LayerClipping, &context);
    assert!(!app.documents.active_session.document.layers[3].clipped);
    assert!(app.documents.active_session.document.validate());
}

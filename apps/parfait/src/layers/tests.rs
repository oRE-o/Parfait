use super::*;
use parfait_core::Document;
use std::sync::Arc;

#[test]
fn nested_layer_text_stays_after_thumbnails_at_narrow_widths_and_maximum_depth()
{
    let directory = tempfile::tempdir().unwrap();
    let mut app = Parfait::open_legacy(directory.path().to_owned()).unwrap();
    for depth in [1, 8, 61] {
        for masked in [false, true] {
            app.documents.active_session.document = Document::new(8, 8);
            app.documents
                .active_session
                .document
                .group_layers_in_folder(2, "Folder".into())
                .unwrap();
            for _ in 1..depth {
                app.documents
                    .active_session
                    .document
                    .create_layer_folder("Nested".into())
                    .unwrap();
            }
            app.documents.active_session.document.add_layer();
            let active = app.documents.active_session.document.active;
            assert_eq!(app.layer_depth_and_hidden(active), (depth, false));
            if masked {
                assert!(
                    app.documents
                        .active_session
                        .document
                        .create_layer_mask(active, 255)
                );
            }
            let before = app.documents.active_session.document.clone();
            for width in [180., 240., 400.] {
                let context = egui::Context::default();
                crate::theme::apply(&context);
                for _ in 0..2 {
                    let mut output = context.run_ui(
                        egui::RawInput {
                            screen_rect: Some(Rect::from_min_size(
                                Pos2::ZERO,
                                egui::vec2(800., 800.),
                            )),
                            ..Default::default()
                        },
                        |ui| {
                            egui::Panel::left("nested_layers")
                                .exact_size(width)
                                .show(ui, |ui| {
                                    app.palette(
                                        ui,
                                        crate::workspace::Palette::Layers,
                                        700.,
                                    )
                                });
                        },
                    );
                    output.textures_delta.clear();
                }
                let thumbnail = context
                    .data(|data| {
                        data.get_temp::<Rect>(egui::Id::new((
                            "layer_target",
                            active,
                            masked,
                        )))
                    })
                    .unwrap();
                let label = row(&context, "layer_label", active);
                assert!(label.left() >= thumbnail.right() + 4.);
                assert!(label.right() < width);
                assert!(label.width() > 0.);
                assert!(!label.intersects(thumbnail));
                assert!(
                    app.documents
                        .active_session
                        .document
                        .same_edit_state(&before)
                );
            }
        }
    }
}

#[test]
fn folder_disclosure_click_selects_parent_without_editing_and_navigation_reveals_child()
 {
    let directory = tempfile::tempdir().unwrap();
    let mut app = Parfait::open_legacy(directory.path().to_owned()).unwrap();
    app.documents.active_session.document = Document::new(8, 8);
    let folder = app
        .documents
        .active_session
        .document
        .group_layers_in_folder(2, "Folder".into())
        .unwrap();
    app.select_layer(1);
    let before = app.documents.active_session.document.clone();
    let context = egui::Context::default();
    crate::theme::apply(&context);
    let mut time = 0.;
    for _ in 0..2 {
        frame(&mut app, &context, vec![], &mut time);
    }
    let fold = context
        .data(|data| {
            data.get_temp::<Rect>(egui::Id::new(("folder_fold", folder)))
        })
        .unwrap()
        .center();
    for pressed in [true, false] {
        frame(
            &mut app,
            &context,
            vec![
                egui::Event::PointerMoved(fold),
                pointer(fold, pressed, Default::default()),
            ],
            &mut time,
        );
    }
    assert_eq!(app.documents.active_session.document.active, 2);
    assert_eq!(app.layer_depth_and_hidden(1), (1, true));
    assert_eq!(app.documents.active_session.history.rows().count(), 0);
    app.select_layer(1);
    assert_eq!(app.layer_depth_and_hidden(1), (1, false));
    assert!(app.documents.active_session.document.same_edit_state(&before));
    assert!(app.documents.active_session.layer_ui.list.collapsed.is_empty());
}

#[test]
fn tree_drag_moves_into_folder_on_release_and_preserves_pixels_and_undo() {
    let directory = tempfile::tempdir().unwrap();
    let mut app = Parfait::open_legacy(directory.path().to_owned()).unwrap();
    app.documents.active_session.document = Document::new(8, 8);
    app.documents.active_session.document.add_layer();
    let folder = app
        .documents
        .active_session
        .document
        .group_layers_in_folder(2, "Folder".into())
        .unwrap();
    let before = app.documents.active_session.document.clone();
    let moved = app.documents.active_session.document.layers[3].id;
    let context = egui::Context::default();
    crate::theme::apply(&context);
    let mut time = 0.;
    for _ in 0..2 {
        frame(&mut app, &context, vec![], &mut time);
    }
    let start = row(&context, "layer_row", 3).center();
    let target = row(&context, "layer_row", 2).center();
    frame(
        &mut app,
        &context,
        vec![
            egui::Event::PointerMoved(start),
            pointer(start, true, Default::default()),
            egui::Event::PointerMoved(target),
        ],
        &mut time,
    );
    assert_eq!(app.documents.active_session.document.layers, before.layers);
    frame(
        &mut app,
        &context,
        vec![pointer(target, false, Default::default())],
        &mut time,
    );
    assert_eq!(app.documents.active_session.document.layers[2].id, moved);
    assert_eq!(
        app.documents.active_session.document.layers[2].parent,
        Some(folder)
    );
    assert_eq!(app.documents.active_session.document.active, 2);
    assert!(Arc::ptr_eq(
        &app.documents.active_session.document.layers[2].pixels,
        &before.layers[3].pixels
    ));
    assert!(app.documents.active_session.document.validate());
    assert_eq!(app.documents.active_session.history.rows().count(), 2);
    app.undo();
    assert_eq!(app.documents.active_session.document.layers, before.layers);
    app.redo();
    assert_eq!(
        app.documents.active_session.document.layers[2].parent,
        Some(folder)
    );
}

#[test]
fn solo_child_keeps_ancestors_visible_and_restores_original_flags() {
    let directory = tempfile::tempdir().unwrap();
    let mut app = Parfait::open_legacy(directory.path().to_owned()).unwrap();
    app.documents.active_session.document = Document::new(8, 8);
    app.documents.active_session.document.add_layer();
    app.documents
        .active_session
        .document
        .group_layers_in_folder(2, "Folder".into())
        .unwrap();
    app.documents.active_session.document.layers[2].visible = false;
    let before = app.documents.active_session.document.clone();
    app.solo_layer(1);
    assert_eq!(
        app.documents
            .active_session
            .document
            .layers
            .iter()
            .map(|layer| layer.visible)
            .collect::<Vec<_>>(),
        [false, true, true, false]
    );
    app.solo_layer(1);
    assert!(app.documents.active_session.document.same_edit_state(&before));
}

fn frame(
    app: &mut Parfait,
    ctx: &egui::Context,
    events: Vec<egui::Event>,
    time: &mut f64,
) {
    *time += 0.03;
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(
                Pos2::ZERO,
                egui::vec2(960., 640.),
            )),
            time: Some(*time),
            events,
            ..Default::default()
        },
        |ui| {
            app.prepare_layer_list(ui.ctx());
            egui::Panel::left("test_layers").exact_size(240.).show(ui, |ui| {
                app.palette(ui, crate::workspace::Palette::Layers, 500.);
            });
            egui::CentralPanel::default().show(ui, |ui| app.canvas(ui));
            app.settle_layer_edit(ui.ctx());
        },
    );
    output.textures_delta.clear();
}
fn row(ctx: &egui::Context, kind: &str, layer: usize) -> Rect {
    ctx.data(|data| data.get_temp::<Rect>(egui::Id::new((kind, layer))))
        .unwrap()
}
fn pointer(
    pos: Pos2,
    pressed: bool,
    modifiers: egui::Modifiers,
) -> egui::Event {
    egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers,
    }
}
fn setup(app: &mut Parfait, ctx: &egui::Context, time: &mut f64) {
    app.documents.active_session.document = Document::new(16, 16);
    while app.documents.active_session.document.layers.len() < 4 {
        app.documents.active_session.document.add_layer();
    }
    for (index, layer) in
        app.documents.active_session.document.layers.iter_mut().enumerate()
    {
        layer.name = format!("Layer {index}");
    }
    app.documents.active_session.document.active = 2;
    crate::theme::apply(ctx);
    frame(app, ctx, vec![], time);
    frame(app, ctx, vec![], time);
}

#[test]
fn batched_drag_inserts_without_copying_pixels_and_undo_restores_order() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = Parfait::open_legacy(dir.path().to_owned()).unwrap();
    let ctx = egui::Context::default();
    let mut time = 0.;
    setup(&mut app, &ctx, &mut time);
    let before = app.documents.active_session.document.clone();
    let start = row(&ctx, "layer_row", 2).center();
    let target = row(&ctx, "layer_row", 0).center_bottom() - egui::vec2(0., 2.);
    frame(
        &mut app,
        &ctx,
        vec![
            egui::Event::PointerMoved(start),
            pointer(start, true, Default::default()),
            egui::Event::PointerMoved(target),
            pointer(target, false, Default::default()),
        ],
        &mut time,
    );
    assert_eq!(
        app.documents
            .active_session
            .document
            .layers
            .iter()
            .map(|layer| layer.name.as_str())
            .collect::<Vec<_>>(),
        ["Layer 2", "Layer 0", "Layer 1", "Layer 3"]
    );
    assert_eq!(app.documents.active_session.document.active, 0);
    assert!(Arc::ptr_eq(
        &app.documents.active_session.document.layers[0].pixels,
        &before.layers[2].pixels
    ));
    assert_eq!(app.documents.active_session.history.rows().count(), 2);
    app.undo();
    assert_eq!(app.documents.active_session.document.layers, before.layers);
    app.redo();
    assert_eq!(app.documents.active_session.document.layers[0].name, "Layer 2");
}

#[test]
fn drag_preview_cancel_outside_and_stale_state_preserve_document_and_redo() {
    for cancel in 0..4 {
        let dir = tempfile::tempdir().unwrap();
        let mut app = Parfait::open_legacy(dir.path().to_owned()).unwrap();
        let ctx = egui::Context::default();
        let mut time = 0.;
        setup(&mut app, &ctx, &mut time);
        app.execute_command(Command::LayerLock, &ctx);
        app.undo();
        frame(&mut app, &ctx, vec![], &mut time);
        let before = app.documents.active_session.document.clone();
        let start = row(&ctx, "layer_row", 2).center();
        let target = row(&ctx, "layer_row", 0).center();
        frame(
            &mut app,
            &ctx,
            vec![
                egui::Event::PointerMoved(start),
                pointer(start, true, Default::default()),
            ],
            &mut time,
        );
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(target)],
            &mut time,
        );
        assert_eq!(app.documents.active_session.document, before);
        let events = match cancel {
            0 => vec![egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Default::default(),
            }],
            1 => {
                vec![pointer(egui::pos2(800., 300.), false, Default::default())]
            }
            2 => vec![egui::Event::WindowFocused(false)],
            _ => {
                app.documents.active_session.document.touch();
                vec![]
            }
        };
        frame(&mut app, &ctx, events, &mut time);
        frame(
            &mut app,
            &ctx,
            vec![pointer(target, false, Default::default())],
            &mut time,
        );
        assert_eq!(app.documents.active_session.document.layers, before.layers);
        assert_eq!(app.documents.active_session.document.active, before.active);
        assert!(app.documents.active_session.history.can_redo());
        assert!(app.documents.active_session.layer_ui.list.drag.is_none());
        assert!(app.documents.active_session.editing.stroke.is_none());
    }
}

#[test]
fn check_shift_and_group_actions_keep_active_and_have_one_history_entry() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = Parfait::open_legacy(dir.path().to_owned()).unwrap();
    let ctx = egui::Context::default();
    let mut time = 0.;
    setup(&mut app, &ctx, &mut time);
    let check = row(&ctx, "layer_check", 0).center();
    frame(
        &mut app,
        &ctx,
        vec![
            egui::Event::PointerMoved(check),
            pointer(check, true, Default::default()),
            pointer(check, false, Default::default()),
        ],
        &mut time,
    );
    assert_eq!(app.selected_layer_mask(), 0b0101);
    assert_eq!(app.documents.active_session.document.active, 2);
    assert_eq!(app.documents.active_session.history.rows().count(), 0);
    app.execute_command(Command::LayerLock, &ctx);
    assert!(
        app.documents.active_session.document.layers[0].locked
            && app.documents.active_session.document.layers[2].locked
    );
    assert!(!app.documents.active_session.document.layers[1].locked);
    assert_eq!(app.documents.active_session.history.rows().count(), 2);
    app.execute_command(Command::DuplicateLayer, &ctx);
    assert_eq!(app.documents.active_session.document.layers.len(), 6);
    assert_eq!(app.selected_layer_mask().count_ones(), 2);
    assert_eq!(app.documents.active_session.history.rows().count(), 3);
    app.execute_command(Command::DeleteLayer, &ctx);
    assert_eq!(app.documents.active_session.document.layers.len(), 4);
    assert_eq!(app.documents.active_session.history.rows().count(), 4);
    app.layer_click(1, Default::default(), false);
    app.layer_click(
        3,
        egui::Modifiers { shift: true, ..Default::default() },
        false,
    );
    assert_eq!(app.selected_layer_mask(), 0b1110);
    assert_eq!(app.documents.active_session.document.active, 3);
    app.execute_command(Command::MoveLayerDown, &ctx);
    assert_eq!(app.selected_layer_mask(), 0b0111);
    assert_eq!(app.documents.active_session.document.active, 2);
    app.undo();
    assert_eq!(
        app.selected_layer_mask(),
        1 << app.documents.active_session.document.active
    );
}

#[test]
fn double_click_renames_inline_enter_commits_and_escape_restores() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = Parfait::open_legacy(dir.path().to_owned()).unwrap();
    let ctx = egui::Context::default();
    let mut time = 0.;
    setup(&mut app, &ctx, &mut time);
    let name = row(&ctx, "layer_label", 2).center();
    for _ in 0..2 {
        frame(
            &mut app,
            &ctx,
            vec![
                egui::Event::PointerMoved(name),
                pointer(name, true, Default::default()),
            ],
            &mut time,
        );
        frame(
            &mut app,
            &ctx,
            vec![pointer(name, false, Default::default())],
            &mut time,
        );
    }
    assert_eq!(
        app.documents.active_session.layer_ui.list.rename,
        Some(app.documents.active_session.document.layers[2].id)
    );
    frame(&mut app, &ctx, vec![], &mut time);
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::Text("Linework".into())],
        &mut time,
    );
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::Key {
            key: egui::Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Default::default(),
        }],
        &mut time,
    );
    assert_eq!(
        app.documents.active_session.document.layers[2].name,
        "Linework"
    );
    assert!(app.documents.active_session.layer_ui.list.rename.is_none());
    assert_eq!(app.documents.active_session.history.rows().count(), 2);
    app.begin_layer_rename(2);
    frame(&mut app, &ctx, vec![], &mut time);
    frame(&mut app, &ctx, vec![egui::Event::Text("Discard".into())], &mut time);
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Default::default(),
        }],
        &mut time,
    );
    assert_eq!(
        app.documents.active_session.document.layers[2].name,
        "Linework"
    );
    assert_eq!(app.documents.active_session.history.rows().count(), 2);
    app.undo();
    assert_eq!(app.documents.active_session.document.layers[2].name, "Layer 2");
}

#[test]
fn multi_drag_keeps_relative_order_and_refuses_delete_all_atomically() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = Parfait::open_legacy(dir.path().to_owned()).unwrap();
    app.documents.active_session.document = Document::new(4, 4);
    while app.documents.active_session.document.layers.len() < 6 {
        app.documents.active_session.document.add_layer();
    }
    for (index, layer) in
        app.documents.active_session.document.layers.iter_mut().enumerate()
    {
        layer.name = index.to_string();
    }
    app.documents.active_session.document.active = 1;
    app.set_layer_selection(0b001010, 1);
    app.reorder_layer_mask(0b001010, 6);
    assert_eq!(
        app.documents
            .active_session
            .document
            .layers
            .iter()
            .map(|layer| layer.name.as_str())
            .collect::<Vec<_>>(),
        ["0", "2", "4", "5", "1", "3"]
    );
    assert_eq!(app.documents.active_session.document.active, 4);
    assert_eq!(app.selected_layer_mask(), 0b110000);
    let before = app.documents.active_session.document.clone();
    app.reorder_layer_mask(app.selected_layer_mask(), 6);
    assert_eq!(app.documents.active_session.document, before);
    app.set_layer_selection(0b111111, 4);
    let ctx = egui::Context::default();
    app.execute_command(Command::DeleteLayer, &ctx);
    assert_eq!(app.documents.active_session.document, before);
}

#[test]
fn modifier_row_clicks_toggle_and_range_select_without_emptying_the_target() {
    for control in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let mut app = Parfait::open_legacy(dir.path().to_owned()).unwrap();
        let ctx = egui::Context::default();
        let mut time = 0.;
        setup(&mut app, &ctx, &mut time);
        let toggle = egui::Modifiers {
            command: !control,
            ctrl: control,
            ..Default::default()
        };
        for (layer, mask, active) in
            [(1, 0b0110, 2), (2, 0b0010, 1), (1, 0b0010, 1)]
        {
            let position = row(&ctx, "layer_row", layer).center();
            frame(
                &mut app,
                &ctx,
                vec![
                    egui::Event::PointerMoved(position),
                    pointer(position, true, toggle),
                    pointer(position, false, toggle),
                ],
                &mut time,
            );
            assert_eq!(app.selected_layer_mask(), mask);
            assert_eq!(app.documents.active_session.document.active, active);
        }
        let position = row(&ctx, "layer_row", 3).center();
        let shift = egui::Modifiers { shift: true, ..Default::default() };
        frame(
            &mut app,
            &ctx,
            vec![
                egui::Event::PointerMoved(position),
                pointer(position, true, shift),
                pointer(position, false, shift),
            ],
            &mut time,
        );
        assert_eq!(app.selected_layer_mask(), 0b1110);
        assert_eq!(app.documents.active_session.document.active, 3);
        assert_eq!(app.documents.active_session.history.rows().count(), 0);
        assert!(app.documents.active_session.layer_ui.list.rename.is_none());
    }
}

#[test]
fn grouped_duplicate_is_preflighted_before_partial_growth_and_solo_restores_flags()
 {
    let dir = tempfile::tempdir().unwrap();
    let mut app = Parfait::open_legacy(dir.path().to_owned()).unwrap();
    app.documents.active_session.document = Document::new(2, 2);
    while app.documents.active_session.document.layers.len() < 63 {
        app.documents.active_session.document.add_layer();
    }
    let active = app.documents.active_session.document.active;
    app.set_layer_selection((1 << active) | 1, active);
    let before = app.documents.active_session.document.clone();
    app.execute_command(Command::DuplicateLayer, &egui::Context::default());
    assert_eq!(app.documents.active_session.document, before);
    assert_eq!(app.documents.active_session.history.rows().count(), 0);
    app.documents.active_session.document.layers[3].visible = false;
    let visibility: Vec<_> = app
        .documents
        .active_session
        .document
        .layers
        .iter()
        .map(|layer| layer.visible)
        .collect();
    app.solo_layer(1);
    assert_eq!(
        app.documents
            .active_session
            .document
            .layers
            .iter()
            .filter(|layer| layer.visible)
            .count(),
        1
    );
    app.solo_layer(1);
    assert_eq!(
        app.documents
            .active_session
            .document
            .layers
            .iter()
            .map(|layer| layer.visible)
            .collect::<Vec<_>>(),
        visibility
    );
}

#[test]
fn dragging_at_viewport_edge_scrolls_and_narrow_rows_do_not_escape_the_palette()
{
    let dir = tempfile::tempdir().unwrap();
    let mut app = Parfait::open_legacy(dir.path().to_owned()).unwrap();
    let ctx = egui::Context::default();
    let mut time = 0.;
    setup(&mut app, &ctx, &mut time);
    while app.documents.active_session.document.layers.len() < 24 {
        app.documents.active_session.document.add_layer();
    }
    frame(&mut app, &ctx, vec![], &mut time);
    let start = row(&ctx, "layer_row", 23).center();
    let viewport = ctx
        .data(|data| {
            data.get_temp::<Rect>(egui::Id::new("layer_list_viewport"))
        })
        .unwrap();
    let edge = viewport.center_bottom() - egui::vec2(0., 2.);
    frame(
        &mut app,
        &ctx,
        vec![
            egui::Event::PointerMoved(start),
            pointer(start, true, Default::default()),
            egui::Event::PointerMoved(edge),
        ],
        &mut time,
    );
    for _ in 0..10 {
        frame(&mut app, &ctx, vec![], &mut time);
    }
    assert!(row(&ctx, "layer_row", 23).top() < start.y - 21.);
    frame(&mut app, &ctx, vec![egui::Event::WindowFocused(false)], &mut time);
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(
                Pos2::ZERO,
                egui::vec2(180., 600.),
            )),
            ..Default::default()
        },
        |ui| {
            app.palette(ui, crate::workspace::Palette::Layers, 500.);
        },
    );
    output.textures_delta.clear();
    assert!(row(&ctx, "layer_label", 23).right() < 180.);
}

#[test]
fn group_drag_in_docked_workspace_uses_release_position_without_a_move_event() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = Parfait::open_legacy(dir.path().to_owned()).unwrap();
    app.documents.active_session.document = Document::new(8, 8);
    app.documents.active_session.document.add_layer();
    app.documents.active_session.document.add_layer();
    let ctx = egui::Context::default();
    crate::theme::apply(&ctx);
    let mut time = 0.;
    let mut run = |app: &mut Parfait, events| {
        time += 0.1;
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(
                    Pos2::ZERO,
                    egui::vec2(1440., 920.),
                )),
                time: Some(time),
                events,
                ..Default::default()
            },
            |ui| {
                app.prepare_layer_list(ui.ctx());
                app.workspace_ui(ui);
            },
        );
        output.textures_delta.clear();
    };
    run(&mut app, vec![]);
    run(&mut app, vec![]);
    let check = row(&ctx, "layer_check", 2).center();
    run(
        &mut app,
        vec![
            egui::Event::PointerMoved(check),
            pointer(check, true, Default::default()),
        ],
    );
    run(&mut app, vec![pointer(check, false, Default::default())]);
    assert_eq!(app.selected_layer_mask(), 0b1100);
    let start = row(&ctx, "layer_row", 3).center();
    let end = row(&ctx, "layer_row", 1).center_bottom() - egui::vec2(0., 1.);
    let original = app.documents.active_session.document.clone();
    run(
        &mut app,
        vec![
            egui::Event::PointerMoved(start),
            pointer(start, true, Default::default()),
        ],
    );
    assert!(app.documents.active_session.layer_ui.list.drag.is_some());
    run(&mut app, vec![pointer(end, false, Default::default())]);
    assert_eq!(app.selected_layer_mask(), 0b0110);
    assert_eq!(
        app.documents
            .active_session
            .document
            .layers
            .iter()
            .map(|layer| layer.name.as_str())
            .collect::<Vec<_>>(),
        ["Paper", "Layer 2", "Layer 3", "Layer 1"]
    );
    assert_eq!(app.documents.active_session.history.rows().count(), 2);
    app.undo();
    assert_eq!(app.documents.active_session.document.layers, original.layers);
}

#[test]
fn hiding_a_rename_editor_commits_once_and_popup_clicks_do_not_start_row_drags()
{
    let dir = tempfile::tempdir().unwrap();
    let mut app = Parfait::open_legacy(dir.path().to_owned()).unwrap();
    let ctx = egui::Context::default();
    let mut time = 0.;
    setup(&mut app, &ctx, &mut time);
    app.begin_layer_rename(2);
    frame(&mut app, &ctx, vec![], &mut time);
    frame(&mut app, &ctx, vec![egui::Event::Text("Renamed".into())], &mut time);
    app.hide_panels = true;
    app.prepare_layer_list(&ctx);
    assert!(app.documents.active_session.layer_ui.list.rename.is_none());
    assert_eq!(app.documents.active_session.history.rows().count(), 2);
    app.undo();
    assert_eq!(app.documents.active_session.document.layers[2].name, "Layer 2");
    app.hide_panels = false;
    frame(&mut app, &ctx, vec![], &mut time);
    let target = row(&ctx, "layer_row", 2).center();
    let mut draw = |events| {
        time += 0.1;
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(
                    Pos2::ZERO,
                    egui::vec2(960., 640.),
                )),
                time: Some(time),
                events,
                ..Default::default()
            },
            |ui| {
                egui::Area::new(egui::Id::new("overlapping_popup"))
                    .order(egui::Order::Foreground)
                    .fixed_pos(target - egui::vec2(30., 20.))
                    .show(ui.ctx(), |ui| {
                        ui.add_sized(
                            [100., 40.],
                            egui::Button::new("Popup action"),
                        );
                    });
                app.layer_rows(ui, 500.);
            },
        );
        output.textures_delta.clear();
    };
    draw(vec![]);
    draw(vec![]);
    draw(vec![
        egui::Event::PointerMoved(target),
        pointer(target, true, Default::default()),
    ]);
    assert!(app.documents.active_session.layer_ui.list.drag.is_none());
}

#[test]
fn multi_document_layer_drag_cancels_before_switch_and_inline_name_blocks_switch()
 {
    let dir = tempfile::tempdir().unwrap();
    let mut app = Parfait::open_legacy(dir.path().to_owned()).unwrap();
    let ctx = egui::Context::default();
    crate::theme::apply(&ctx);
    let mut time = 0.;
    setup(&mut app, &ctx, &mut time);
    app.enable_document_tabs().unwrap();
    let a = app.documents.active_id().unwrap();
    let b = app.new_document_tab(Document::new(16, 16)).unwrap();
    app.activate_document(a, true);
    frame(&mut app, &ctx, vec![], &mut time);
    let start = row(&ctx, "layer_row", 3).center();
    let end = row(&ctx, "layer_row", 1).center_bottom();
    let before_a = app.documents.active_session.document.clone();
    frame(
        &mut app,
        &ctx,
        vec![
            egui::Event::PointerMoved(start),
            pointer(start, true, Default::default()),
        ],
        &mut time,
    );
    frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)], &mut time);
    assert!(app.documents.active_session.layer_ui.list.drag.is_some());
    assert!(app.activate_document(b, true));
    assert!(app.documents.get(a).unwrap().layer_ui.list.drag.is_none());
    let before_b = app.documents.active_session.document.clone();
    frame(
        &mut app,
        &ctx,
        vec![pointer(end, false, Default::default())],
        &mut time,
    );
    assert_eq!(app.documents.active_session.document, before_b);
    assert_eq!(app.documents.get(a).unwrap().document.layers, before_a.layers);
    app.begin_layer_rename(1);
    assert!(!app.activate_document(a, true));
    app.documents.active_session.layer_ui.list.cancel_pointer();
    assert!(app.activate_document(a, true));
}

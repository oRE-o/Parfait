use super::*;
use parfait_core::Document;

fn frame(app: &mut Parfait, ctx: &egui::Context, events: Vec<egui::Event>, width: f32) {
    ctx.data_mut(|data| {
        data.remove::<TopMenu>(egui::Id::new("open_top_menu"));
    });
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(width, 640.),
            )),
            events,
            ..Default::default()
        },
        |ui| {
            app.prepare_layer_list(ui.ctx());
            app.shortcuts(ui.ctx());
            ui.add_enabled_ui(!app.dialog_open(), |ui| {
                app.top_panel(ui);
                egui::CentralPanel::default().show(ui, |ui| app.canvas(ui));
            });
        },
    );
    output.textures_delta.clear();
}

fn rect(ctx: &egui::Context, id: impl std::hash::Hash + std::fmt::Debug) -> egui::Rect {
    ctx.data(|data| data.get_temp::<egui::Rect>(egui::Id::new(id)))
        .unwrap()
}

fn click(app: &mut Parfait, ctx: &egui::Context, target: egui::Rect) {
    for pressed in [true, false] {
        frame(
            app,
            ctx,
            vec![
                egui::Event::PointerMoved(target.center()),
                egui::Event::PointerButton {
                    pos: target.center(),
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: Default::default(),
                },
            ],
            960.,
        );
    }
    frame(app, ctx, vec![], 960.);
}

fn escape(app: &mut Parfait, ctx: &egui::Context) {
    for pressed in [true, false] {
        frame(
            app,
            ctx,
            vec![egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed,
                repeat: false,
                modifiers: Default::default(),
            }],
            960.,
        );
    }
}

fn open(app: &mut Parfait, ctx: &egui::Context, menu: TopMenu) {
    click(app, ctx, rect(ctx, ("top_menu", menu)));
    assert_eq!(
        ctx.data(|data| data.get_temp::<TopMenu>(egui::Id::new("open_top_menu"))),
        Some(menu)
    );
}

fn reveal(
    app: &mut Parfait,
    ctx: &egui::Context,
    path: egui::Id,
    target: impl Fn(&egui::Context) -> egui::Rect,
) -> egui::Rect {
    for _ in 0..80 {
        let viewport = rect(ctx, ("catalog_viewport", path));
        let item = target(ctx);
        if viewport.shrink(1.).contains(item.center()) {
            frame(
                app,
                ctx,
                vec![egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    phase: egui::TouchPhase::End,
                    delta: egui::Vec2::ZERO,
                    modifiers: Default::default(),
                }],
                960.,
            );
            return target(ctx);
        }
        frame(
            app,
            ctx,
            vec![
                egui::Event::PointerMoved(viewport.center()),
                egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    phase: egui::TouchPhase::Start,
                    delta: egui::Vec2::ZERO,
                    modifiers: Default::default(),
                },
                egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    phase: egui::TouchPhase::Move,
                    delta: egui::vec2(
                        0.,
                        (viewport.center().y - item.center().y).clamp(-240., 240.),
                    ),
                    modifiers: Default::default(),
                },
            ],
            960.,
        );
    }
    panic!("Menu entry did not become visible: {:?}", target(ctx));
}

fn root_path(menu: TopMenu) -> egui::Id {
    egui::Id::new(("catalog_root", menu))
}

fn group_rect(ctx: &egui::Context, id: egui::Id) -> egui::Rect {
    ctx.data(|data| {
        data.get_temp::<(egui::Rect, bool)>(egui::Id::new(("catalog_group", id)))
            .unwrap()
            .0
    })
}

fn open_group(app: &mut Parfait, ctx: &egui::Context, path: egui::Id, index: usize) -> egui::Id {
    let id = path.with(index);
    let target = reveal(app, ctx, path, |ctx| group_rect(ctx, id));
    if ctx.data(|data| {
        data.get_temp::<(egui::Rect, bool)>(egui::Id::new(("catalog_group", id)))
            .unwrap()
            .1
    }) {
        return id;
    }
    click(app, ctx, target);
    assert!(ctx.data(|data| {
        data.get_temp::<(egui::Rect, bool)>(egui::Id::new(("catalog_group", id)))
            .unwrap()
            .1
    }));
    id
}

#[test]
fn eight_korean_menus_fit_minimum_width_with_separate_save_status_and_glyphs() {
    assert_eq!(
        TopMenu::ALL.map(TopMenu::label),
        [
            "파일",
            "편집",
            "레이어",
            "선택",
            "표시",
            "필터",
            "창",
            "도움말"
        ]
    );
    let dir = tempfile::tempdir().unwrap();
    let mut app = Parfait::open(dir.path().to_owned()).unwrap();
    app.doc = Document::new(8, 8);
    let before = app.doc.clone();
    let ctx = egui::Context::default();
    crate::theme::apply(&ctx);
    for width in [960., 1440.] {
        frame(&mut app, &ctx, vec![], width);
        frame(&mut app, &ctx, vec![], width);
        let mut right = 0.;
        for menu in TopMenu::ALL {
            let target = rect(&ctx, ("top_menu", menu));
            assert!(target.left() >= right);
            assert!(target.right() < width);
            right = target.right();
            assert!(ctx.fonts_mut(|fonts| {
                fonts.has_glyphs(&egui::FontId::proportional(13.), menu.label())
            }));
        }
        let status = rect(&ctx, "menu_save_status");
        assert!(status.left() > right);
        assert!(status.right() <= width);
    }
    assert_eq!(app.doc, before);
    assert_eq!(app.history.rows().count(), 0);
    assert_eq!(Command::ALL.len(), 78);
}

#[test]
fn every_menu_opens_and_escape_closes_without_changing_artwork() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = Parfait::open(dir.path().to_owned()).unwrap();
    app.doc = Document::new(8, 8);
    let before = app.doc.clone();
    let ctx = egui::Context::default();
    crate::theme::apply(&ctx);
    frame(&mut app, &ctx, vec![], 960.);
    frame(&mut app, &ctx, vec![], 960.);
    for menu in TopMenu::ALL {
        open(&mut app, &ctx, menu);
        assert!(egui::Popup::is_any_open(&ctx));
        let caption = rect(&ctx, ("catalog_caption", menu));
        let viewport = rect(&ctx, ("catalog_viewport", root_path(menu)));
        assert!(viewport.top() >= caption.bottom());
        escape(&mut app, &ctx);
        assert!(!egui::Popup::is_any_open(&ctx));
        assert_eq!(app.doc, before);
        assert!(app.stroke.is_none());
    }
}

#[test]
fn localized_menu_commands_reuse_transactions_and_view_window_toggles() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = Parfait::open(dir.path().to_owned()).unwrap();
    app.doc = Document::new(8, 8);
    let before = app.doc.clone();
    let ctx = egui::Context::default();
    crate::theme::apply(&ctx);
    frame(&mut app, &ctx, vec![], 960.);
    frame(&mut app, &ctx, vec![], 960.);
    open(&mut app, &ctx, TopMenu::Layer);
    click(
        &mut app,
        &ctx,
        rect(&ctx, ("menu_item", Command::NewLayer.label())),
    );
    assert_eq!(app.doc.layers.len(), 3);
    assert_eq!(app.history.rows().count(), 2);
    assert!(!egui::Popup::is_any_open(&ctx));
    open(&mut app, &ctx, TopMenu::Edit);
    click(
        &mut app,
        &ctx,
        rect(&ctx, ("menu_item", Command::Undo.label())),
    );
    assert_eq!(app.doc.layers, before.layers);
    assert!(app.history.can_redo());
    let before = app.doc.clone();
    open(&mut app, &ctx, TopMenu::Window);
    let visible = app
        .workspace
        .panels
        .iter()
        .find(|placement| placement.palette == crate::workspace::Palette::Layers)
        .unwrap()
        .visible;
    let target = reveal(&mut app, &ctx, root_path(TopMenu::Window), |ctx| {
        rect(ctx, ("command_toggle", Command::PaletteLayers))
    });
    click(&mut app, &ctx, target);
    assert_eq!(
        app.workspace
            .panels
            .iter()
            .find(|placement| placement.palette == crate::workspace::Palette::Layers)
            .unwrap()
            .visible,
        !visible
    );
    escape(&mut app, &ctx);
    open(&mut app, &ctx, TopMenu::View);
    let rulers = app.display.rulers;
    let target = reveal(&mut app, &ctx, root_path(TopMenu::View), |ctx| {
        rect(ctx, ("command_toggle", Command::RulerBar))
    });
    click(&mut app, &ctx, target);
    assert_eq!(app.display.rulers, !rulers);
    escape(&mut app, &ctx);
    assert_eq!(app.doc, before);
    assert!(app.history.can_redo());
    assert!(!app.keymap.hint(Command::Undo).is_empty());
    open(&mut app, &ctx, TopMenu::Help);
    click(
        &mut app,
        &ctx,
        rect(&ctx, ("menu_item", Command::ShortcutGuide.label())),
    );
    assert!(app.show_help);
    assert!(!egui::Popup::is_any_open(&ctx));
    escape(&mut app, &ctx);
    assert!(!app.show_help);
}

#[test]
fn edit_menu_conversions_change_only_content_and_share_history() {
    let directory = tempfile::tempdir().unwrap();
    let mut app = Parfait::open(directory.path().to_owned()).unwrap();
    app.doc = Document::new(8, 8);
    app.doc.layers[1].pixels = std::sync::Arc::new([128, 128, 128, 128].repeat(64));
    app.doc.create_layer_mask(1, 127);
    app.brush.color = [17, 140, 230];
    let before = app.doc.clone();
    let context = egui::Context::default();
    crate::theme::apply(&context);
    frame(&mut app, &context, vec![], 960.);
    frame(&mut app, &context, vec![], 960.);
    for command in [Command::BrightnessToOpacity, Command::ConvertToDrawingColor] {
        open(&mut app, &context, TopMenu::Edit);
        let target = reveal(&mut app, &context, root_path(TopMenu::Edit), |context| {
            rect(context, ("menu_item", command.label()))
        });
        click(&mut app, &context, target);
        assert!(!egui::Popup::is_any_open(&context));
    }
    assert_eq!(
        app.doc.layers[1].pixels.as_slice(),
        [17, 140, 230, 64].repeat(64)
    );
    assert_eq!(app.doc.layers[1].mask, before.layers[1].mask);
    assert_eq!(app.doc.layers[0], before.layers[0]);
    assert_eq!(app.history.rows().count(), 3);
    app.undo();
    app.undo();
    assert!(app.doc.same_edit_state(&before));
}

#[test]
fn unimplemented_selection_and_filter_items_are_disabled_and_never_paint() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = Parfait::open(dir.path().to_owned()).unwrap();
    app.doc = Document::new(8, 8);
    let before = app.doc.clone();
    let ctx = egui::Context::default();
    crate::theme::apply(&ctx);
    frame(&mut app, &ctx, vec![], 960.);
    frame(&mut app, &ctx, vec![], 960.);
    for (menu, label) in [
        (TopMenu::Select, "선택 범위 반전"),
        (TopMenu::Filter, "선명하게"),
    ] {
        open(&mut app, &ctx, menu);
        if menu == TopMenu::Filter {
            let index = catalog::entries(menu)
                .iter()
                .position(|item| matches!(item, catalog::MenuItem::Group("선명하게", _)))
                .unwrap();
            open_group(&mut app, &ctx, root_path(menu), index);
        }
        let (target, enabled) = ctx
            .data(|data| {
                data.get_temp::<(egui::Rect, bool)>(egui::Id::new(("unsupported_menu", label)))
            })
            .unwrap();
        assert!(!enabled);
        click(&mut app, &ctx, target);
        assert_eq!(app.doc, before);
        assert_eq!(app.history.rows().count(), 0);
        assert!(app.stroke.is_none());
        escape(&mut app, &ctx);
    }
}

#[test]
fn static_menu_catalog_is_bounded_unique_and_separates_actions_from_plans() {
    fn check(items: &[catalog::MenuItem], depth: usize) -> (usize, usize, usize) {
        use catalog::MenuItem;
        assert!(!items.is_empty());
        assert!(depth <= 3);
        let mut labels = std::collections::HashSet::new();
        let mut counts = (0, 0, 0);
        for item in items {
            let label = match item {
                MenuItem::Action(command) | MenuItem::Toggle(command) => {
                    assert!(Command::ALL.contains(command));
                    if matches!(item, MenuItem::Toggle(_)) {
                        assert!(
                            matches!(
                                command,
                                Command::LayerVisibility
                                    | Command::LayerLock
                                    | Command::LayerAlphaLock
                                    | Command::LayerClipping
                                    | Command::LayerMaskEnabled
                                    | Command::ShowMaskArea
                                    | Command::RulerBar
                                    | Command::PixelGrid
                                    | Command::FlipHorizontal
                                    | Command::FlipVertical
                                    | Command::HidePalettes
                            ) || command.palette().is_some()
                        );
                    }
                    counts.0 += 1;
                    menu_label(*command)
                }
                MenuItem::Planned(label) => {
                    counts.1 += 1;
                    label
                }
                MenuItem::Group(label, children) => {
                    let children = check(children, depth + 1);
                    counts.0 += children.0;
                    counts.1 += children.1;
                    counts.2 += children.2 + 1;
                    label
                }
                MenuItem::Separator => continue,
            };
            assert!(labels.insert(label), "Duplicate sibling: {label}");
            assert!(!label.is_empty());
            assert!(!label.starts_with('/'));
            assert!(!label.contains("http"));
            for excluded in ["애니메이션", "타임라인", "무비", "오디오", "타임 랩스"]
            {
                assert!(!label.contains(excluded));
            }
        }
        counts
    }
    let mut counts = (0, 0, 0);
    for menu in TopMenu::ALL {
        let current = check(catalog::entries(menu), 0);
        counts.0 += current.0;
        counts.1 += current.1;
        counts.2 += current.2;
    }
    assert_eq!(counts, (69, 325, 41));
    assert_eq!(Command::ALL.len(), 78);
    eprintln!(
        "catalog: {} action routes, {} planned leaves, {} groups",
        counts.0, counts.1, counts.2
    );
}

#[test]
fn layer_organization_menu_routes_execute_active_actions_not_selected_set_moves() {
    for (command, group, destination) in [
        (Command::RenameLayer, "레이어 설정", 2),
        (Command::MoveActiveLayerUp, "정렬", 3),
        (Command::MoveActiveLayerDown, "정렬", 1),
        (Command::MoveActiveLayerToTop, "정렬", 4),
        (Command::MoveActiveLayerToBottom, "정렬", 0),
        (Command::SelectLayerAbove, "편집 대상으로 설정", 3),
        (Command::SelectLayerBelow, "편집 대상으로 설정", 1),
    ] {
        let directory = tempfile::tempdir().unwrap();
        let mut app = Parfait::open(directory.path().to_owned()).unwrap();
        app.doc = Document::new(8, 8);
        for _ in 0..3 {
            app.doc.add_layer();
        }
        app.select_layer(2);
        app.set_layer_selection(0b10101, 0);
        let before = app.doc.clone();
        let active_id = app.doc.layers[2].id;
        let context = egui::Context::default();
        crate::theme::apply(&context);
        frame(&mut app, &context, vec![], 960.);
        frame(&mut app, &context, vec![], 960.);
        open(&mut app, &context, TopMenu::Layer);
        let index = catalog::entries(TopMenu::Layer)
            .iter()
            .position(|item| matches!(item, catalog::MenuItem::Group(label, _) if *label == group))
            .unwrap();
        let path = open_group(&mut app, &context, root_path(TopMenu::Layer), index);
        let target = reveal(&mut app, &context, path, |context| {
            rect(context, ("menu_item", command.label()))
        });
        click(&mut app, &context, target);
        assert!(!egui::Popup::is_any_open(&context));
        assert_eq!(app.doc.active, destination);
        if command == Command::RenameLayer {
            assert!(app.rename_layer.is_some());
            assert_eq!(app.doc, before);
        } else if matches!(
            command,
            Command::SelectLayerAbove | Command::SelectLayerBelow
        ) {
            assert_eq!(app.selected_layer_mask(), 1 << destination);
            assert!(app.doc.same_edit_state(&before));
            assert_eq!(app.history.rows().count(), 0);
        } else {
            assert_eq!(app.doc.layers[destination].id, active_id);
            assert_eq!(app.selected_layer_mask().count_ones(), 3);
            assert_eq!(app.history.rows().count(), 2);
        }
    }
}

#[test]
fn every_catalog_group_opens_and_planned_widgets_remain_disabled_without_edits() {
    fn collect(
        menu: TopMenu,
        items: &'static [catalog::MenuItem],
        chain: Vec<usize>,
        groups: &mut Vec<(TopMenu, Vec<usize>, &'static [catalog::MenuItem])>,
    ) {
        groups.push((menu, chain.clone(), items));
        for (index, item) in items.iter().enumerate() {
            if let catalog::MenuItem::Group(_, children) = item {
                let mut next = chain.clone();
                next.push(index);
                collect(menu, children, next, groups);
            }
        }
    }
    let dir = tempfile::tempdir().unwrap();
    let mut app = Parfait::open(dir.path().to_owned()).unwrap();
    app.doc = Document::new(8, 8);
    let artwork = app.doc.clone();
    app.persist_preferences(true);
    let preferences = app.saved_preferences_json();
    let snapshot: crate::preferences::Preferences = serde_json::from_str(&preferences).unwrap();
    let ctx = egui::Context::default();
    crate::theme::apply(&ctx);
    frame(&mut app, &ctx, vec![], 960.);
    frame(&mut app, &ctx, vec![], 960.);
    let view = (app.view.zoom, app.view.angle, app.view.pan);
    let mut groups = Vec::new();
    for menu in TopMenu::ALL {
        collect(menu, catalog::entries(menu), vec![], &mut groups);
    }
    let mut checked = 0;
    for (menu, chain, items) in groups {
        open(&mut app, &ctx, menu);
        let mut path = root_path(menu);
        for index in chain {
            path = open_group(&mut app, &ctx, path, index);
        }
        for (index, item) in items.iter().enumerate() {
            if let catalog::MenuItem::Planned(_) = item {
                let id = egui::Id::new(("catalog_planned", path.with(index)));
                let (target, enabled) = ctx
                    .data(|data| data.get_temp::<(egui::Rect, bool)>(id))
                    .unwrap();
                assert!(!enabled);
                let viewport = rect(&ctx, ("catalog_viewport", path));
                if viewport.shrink(1.).contains(target.center()) {
                    click(&mut app, &ctx, target);
                }
                checked += 1;
            }
        }
        assert_eq!(app.doc, artwork);
        assert_eq!(app.history.rows().count(), 0);
        assert_eq!(app.saved_preferences_json(), preferences);
        assert!(snapshot.matches(&app));
        assert_eq!((app.view.zoom, app.view.angle, app.view.pan), view);
        assert!(app.stroke.is_none());
        escape(&mut app, &ctx);
        assert!(!egui::Popup::is_any_open(&ctx));
    }
    assert!(checked >= 250);
}

#[test]
fn long_root_menus_scroll_to_the_last_entry_inside_the_viewport() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = Parfait::open(dir.path().to_owned()).unwrap();
    app.doc = Document::new(8, 8);
    let artwork = app.doc.clone();
    let ctx = egui::Context::default();
    crate::theme::apply(&ctx);
    frame(&mut app, &ctx, vec![], 960.);
    frame(&mut app, &ctx, vec![], 960.);
    for menu in [
        TopMenu::Edit,
        TopMenu::Layer,
        TopMenu::View,
        TopMenu::Window,
    ] {
        open(&mut app, &ctx, menu);
        let path = root_path(menu);
        let items = catalog::entries(menu);
        let target = match items.last().unwrap() {
            catalog::MenuItem::Action(command) => reveal(&mut app, &ctx, path, |ctx| {
                rect(ctx, ("menu_item", command.label()))
            }),
            catalog::MenuItem::Group(_, _) => reveal(&mut app, &ctx, path, |ctx| {
                group_rect(ctx, path.with(items.len() - 1))
            }),
            _ => panic!("Unexpected last entry"),
        };
        let viewport = rect(&ctx, ("catalog_viewport", path));
        assert!(viewport.contains(target.center()));
        assert!(viewport.height() <= 528.1);
        assert!(target.right() <= 960.);
        assert!(target.bottom() <= 640.);
        escape(&mut app, &ctx);
    }
    assert_eq!(app.doc, artwork);
}

#[test]
fn nested_layer_flags_and_subtool_details_keep_the_shared_command_path() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = Parfait::open(dir.path().to_owned()).unwrap();
    app.doc = Document::new(8, 8);
    let original = app.doc.clone();
    let ctx = egui::Context::default();
    crate::theme::apply(&ctx);
    frame(&mut app, &ctx, vec![], 960.);
    frame(&mut app, &ctx, vec![], 960.);
    open(&mut app, &ctx, TopMenu::Layer);
    let index = catalog::entries(TopMenu::Layer)
        .iter()
        .position(|item| matches!(item, catalog::MenuItem::Group("레이어 설정", _)))
        .unwrap();
    open_group(&mut app, &ctx, root_path(TopMenu::Layer), index);
    click(
        &mut app,
        &ctx,
        rect(&ctx, ("command_toggle", Command::LayerAlphaLock)),
    );
    assert!(app.doc.layers[1].alpha_locked);
    assert_eq!(app.doc.layers[1].id, original.layers[1].id);
    assert_eq!(app.history.rows().count(), 2);
    escape(&mut app, &ctx);
    app.undo();
    assert_eq!(app.doc.layers, original.layers);
    open(&mut app, &ctx, TopMenu::Window);
    let target = reveal(&mut app, &ctx, root_path(TopMenu::Window), |ctx| {
        rect(ctx, ("menu_item", Command::SubToolDetail.label()))
    });
    click(&mut app, &ctx, target);
    assert!(app.brush_details.open);
    assert!(app.history.can_redo());
    assert_eq!(app.doc.layers, original.layers);
}

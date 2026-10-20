use super::*;
#[test]
fn canceled_capture_guard_releases_job_and_restores_visibility_without_document_changes()
 {
    let ctx = egui::Context::default();
    let busy = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
    {
        let _guard = CaptureGuard {
            ctx: ctx.clone(),
            palettes: Vec::new(),
            hidden: true,
            busy: busy.clone(),
        };
    }
    assert!(!busy.load(Ordering::Acquire));
}
#[test]
fn synthetic_frame_cancel_drops_pixels_and_keeps_brush_and_artwork() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = Parfait::open_legacy(dir.path().to_owned()).unwrap();
    app.documents.active_session.document = parfait_core::Document::new(2, 2);
    let before = app.documents.active_session.document.clone();
    let color = app.brush.color;
    app.screen_picker = Some(ScreenPicker {
        receiver: None,
        frame: Some(CapturedScreen {
            monitors: vec![(1, "Test".into())],
            selected: 1,
            width: 2,
            height: 2,
            pixels: [20, 40, 60, 255].repeat(4),
        }),
        texture: None,
        hidden: false,
        hide_before_capture: false,
        zoom: 1.,
        error: None,
        sample: None,
    });
    let ctx = egui::Context::default();
    crate::theme::apply(&ctx);
    app.screen_picker.as_mut().unwrap().texture = Some(ctx.load_texture(
        "synthetic",
        egui::ColorImage::from_rgba_unmultiplied(
            [2, 2],
            &[20, 40, 60, 255].repeat(4),
        ),
        egui::TextureOptions::NEAREST,
    ));
    let mut warm = ctx
        .run_ui(egui::RawInput::default(), |ui| app.screen_picker_ui(ui.ctx()));
    warm.textures_delta.clear();
    let mut output = ctx.run_ui(
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
        |ui| app.screen_picker_ui(ui.ctx()),
    );
    output.textures_delta.clear();
    assert!(app.screen_picker.is_none());
    assert_eq!(app.brush.color, color);
    assert!(app.documents.active_session.document.same_edit_state(&before));
}

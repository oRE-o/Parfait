use parfait_core::{Brush, BrushTip, Document, GapMode, Point, Stroke};

fn point() -> Point {
    Point { x: 16.25, y: 32.25, pressure: 1. }
}

fn path(brush: Brush, distances: &[f32]) -> (Document, u64) {
    let mut document = Document::new(256, 64);
    let before = document.revision;
    let mut stroke = Stroke::begin(&mut document, brush, point());
    let mut position = point();
    for distance in distances {
        position.x += distance;
        stroke.extend(&mut document, position);
    }
    let count = document.revision - before;
    (document, count)
}

#[test]
fn gap_modes_place_distinct_stamps_and_fixed_value_is_not_lost() {
    for (mode, pixels, count) in [
        (GapMode::Fixed, 25., 5),
        (GapMode::Wide, 5., 21),
        (GapMode::Normal, 2.5, 41),
        (GapMode::Narrow, 1., 101),
    ] {
        let brush = Brush {
            size: 10.,
            spacing: 2.5,
            gap_mode: mode,
            pressure: false,
            ..Default::default()
        };
        assert!(brush.validate());
        assert_eq!(brush.spacing_pixels(), pixels);
        assert_eq!(path(brush.clone(), &[100.]).1, count);
        let mut fixed = brush;
        fixed.gap_mode = GapMode::Fixed;
        assert_eq!(fixed.spacing_pixels(), 25.);
    }
    assert_eq!(
        Brush { size: 1., spacing: 0.01, ..Default::default() }
            .spacing_pixels(),
        0.5
    );
    for spacing in [f32::NAN, 0., 10.1] {
        assert!(!Brush { spacing, ..Default::default() }.validate());
    }
}

#[test]
fn spacing_phase_and_pixels_do_not_depend_on_movement_event_count() {
    for gap_mode in
        [GapMode::Fixed, GapMode::Wide, GapMode::Normal, GapMode::Narrow]
    {
        let brush = Brush {
            size: 10.,
            gap_mode,
            spacing: 1.,
            pressure: false,
            flow: 0.2,
            ..Default::default()
        };
        let (single, count) = path(brush.clone(), &[100.]);
        let (fragmented, fragmented_count) =
            path(brush, &[7., 13., 9., 21., 11., 39.]);
        assert_eq!(count, fragmented_count);
        assert_eq!(single.layers[1].pixels, fragmented.layers[1].pixels);
    }
}

fn spray_frames(frames: &[f32], enabled: bool) -> (Document, u64) {
    let mut document = Document::new(64, 64);
    let mut stroke = Stroke::begin(
        &mut document,
        Brush {
            tip: BrushTip::Soft,
            size: 20.,
            flow: 0.03,
            pressure: false,
            continuous_spray: enabled,
            ..Default::default()
        },
        point(),
    );
    let before = document.revision;
    for elapsed in frames {
        stroke.spray(&mut document, *elapsed, 1.);
    }
    let count = document.revision - before;
    (document, count)
}

#[test]
fn stationary_spray_uses_time_not_frame_count_and_drops_pause_backlog() {
    let (slow, count) = spray_frames(&[0.1; 10], true);
    let (fast, fast_count) = spray_frames(&[1. / 60.; 60], true);
    assert_eq!(count, 30);
    assert_eq!(count, fast_count);
    assert_eq!(slow.layers[1].pixels, fast.layers[1].pixels);
    assert_eq!(spray_frames(&[10.], true).1, 3);
    assert_eq!(spray_frames(&[0.1; 10], false).1, 0);
    assert_eq!(spray_frames(&[f32::NAN, f32::INFINITY, -1., 0.], true).1, 0);
    for spray_rate in [f32::NAN, 0., 121.] {
        assert!(!Brush { spray_rate, ..Default::default() }.validate());
    }
}

#[test]
fn spray_uses_current_pressure_supports_erasing_and_respects_locked_layers() {
    let mut document = Document::new(64, 64);
    let mut stroke = Stroke::begin(
        &mut document,
        Brush {
            size: 30.,
            flow: 0.1,
            continuous_spray: true,
            ..Default::default()
        },
        Point { pressure: 0., ..point() },
    );
    stroke.take_damage();
    let edge = ((32 * 64 + 26) * 4 + 3) as usize;
    assert_eq!(document.layers[1].pixels[edge], 0);
    stroke.spray(&mut document, 0.1, 1.);
    assert!(document.layers[1].pixels[edge] > 0);
    assert!(stroke.take_damage().is_some());
    let before = document.layers[1].pixels[edge];
    let mut erase = Stroke::begin(
        &mut document,
        Brush {
            size: 30.,
            flow: 0.1,
            erase: true,
            pressure: false,
            continuous_spray: true,
            ..Default::default()
        },
        point(),
    );
    erase.spray(&mut document, 0.1, 1.);
    assert!(document.layers[1].pixels[edge] < before);
    document.layers[1].locked = true;
    let revision = document.revision;
    let pixels = document.layers[1].pixels.clone();
    erase.take_damage();
    erase.spray(&mut document, 0.1, 1.);
    assert_eq!(document.revision, revision);
    assert_eq!(document.layers[1].pixels, pixels);
    assert!(erase.take_damage().is_none());
}

use super::*;
fn fixture() -> Document {
    let mut d = Document::new(32, 24);
    let mut p = vec![0; 32 * 24 * 4];
    for y in 3..21 {
        for x in 4..28 {
            let i = (y * 32 + x) * 4;
            p[i..i + 4].copy_from_slice(&[190, 130, 80, 128]);
        }
    }
    d.layers[1].pixels = Arc::new(p);
    d
}
#[test]
fn shading_uses_separate_clipped_layers_and_preserves_base_alpha() {
    let mut d = fixture();
    let before = d.clone();
    assert!(d.add_local_shading(1, LocalShading::default()));
    assert_eq!(d.layers.len(), 4);
    assert!(Arc::ptr_eq(&before.layers[1].pixels, &d.layers[1].pixels));
    assert!(d.layers[2].clipped && d.layers[3].clipped);
    assert_eq!(d.layers[2].blend_mode, BlendMode::Multiply);
    assert_eq!(d.layers[3].blend_mode, BlendMode::Screen);
    let old = before.composite();
    let new = d.composite();
    assert_ne!(old, new);
    for (a, b) in old.as_chunks::<4>().0.iter().zip(new.as_chunks::<4>().0) {
        assert_eq!(a[3], b[3]);
    }
    d.layers[2].visible = false;
    d.layers[3].visible = false;
    assert_eq!(before.composite(), d.composite());
    assert!(d.validate());
}
#[test]
fn opposite_lights_and_banded_point_lights_are_deterministic() {
    let mut a = fixture();
    let mut b = a.clone();
    let mut o = LocalShading::default();
    assert!(a.add_local_shading(1, o));
    o.angle = 135.;
    assert!(b.add_local_shading(1, o));
    assert_ne!(a.composite(), b.composite());
    let mut c = fixture();
    let mut d = c.clone();
    o.bands = 3;
    o.point_light = true;
    assert!(c.add_local_shading(1, o));
    assert!(d.add_local_shading(1, o));
    assert_eq!(c.composite(), d.composite());
}
#[test]
fn empty_selection_locks_invalid_values_and_cancel_are_atomic() {
    let mut d = fixture();
    let before = d.clone();
    let o = LocalShading { radius: f32::NAN, ..Default::default() };
    assert!(!d.add_local_shading(1, o));
    assert_eq!(d, before);
    assert!(!d.add_local_shading_cancellable(
        1,
        LocalShading::default(),
        || true
    ));
    assert_eq!(d, before);
    d.layers[1].locked = true;
    let before = d.clone();
    assert!(!d.add_local_shading(1, LocalShading::default()));
    assert_eq!(d, before);
}
#[test]
fn selection_limits_generated_coverage_and_reference_lines_change_result() {
    let mut d = fixture();
    d.selection = Some(crate::MaskCoverage::Dense(Arc::new(
        (0..32 * 24).map(|i| if i % 32 < 16 { 255 } else { 0 }).collect(),
    )));
    assert!(d.add_local_shading(1, LocalShading::default()));
    for layer in &d.layers[2..] {
        for y in 0..24 {
            for x in 16..32 {
                assert_eq!(layer.pixels[(y * 32 + x) * 4 + 3], 0);
            }
        }
    }
    let mut a = fixture();
    a.add_layer();
    a.layers[2].reference = true;
    let mut ink = vec![0; 32 * 24 * 4];
    for y in 4..20 {
        ink[(y * 32 + 16) * 4..(y * 32 + 16) * 4 + 4]
            .copy_from_slice(&[0, 0, 0, 255]);
    }
    a.layers[2].pixels = Arc::new(ink);
    let mut b = a.clone();
    let mut o = LocalShading::default();
    a.add_local_shading(1, o);
    o.line_influence = 1.;
    b.add_local_shading(1, o);
    assert_ne!(a.composite(), b.composite());
}

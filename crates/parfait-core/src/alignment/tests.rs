use super::*;
fn shapes() -> Document {
    let mut doc = Document::new(12, 10);
    doc.add_layer();
    doc.add_layer();
    for (index, (x, y, w, h)) in
        [(0, 1, 2, 2), (7, 3, 3, 1), (10, 7, 1, 3)].into_iter().enumerate()
    {
        let pixels = Arc::make_mut(&mut doc.layers[index + 1].pixels);
        for py in y..y + h {
            for px in x..x + w {
                let i = (py * 12 + px) * 4;
                pixels[i..i + 4].copy_from_slice(&[100, 150, 200, 255]);
            }
        }
    }
    doc
}
#[test]
fn every_alignment_and_distribution_is_valid_and_preserves_endpoints_or_canvas_reference()
 {
    for mode in [
        Align::Top,
        Align::CenterY,
        Align::Bottom,
        Align::Left,
        Align::CenterX,
        Align::Right,
        Align::DistributeTop,
        Align::DistributeCenterY,
        Align::DistributeBottom,
        Align::DistributeLeft,
        Align::DistributeCenterX,
        Align::DistributeRight,
        Align::SpaceY,
        Align::SpaceX,
    ] {
        let mut doc = shapes();
        let before = doc.clone();
        let plan = doc.alignment_plan(0b1110, mode, None).unwrap();
        doc.translate_layer_pixels(&plan);
        assert!(doc.validate(), "{mode:?}");
        assert!(Arc::ptr_eq(&doc.layers[0].pixels, &before.layers[0].pixels));
        let bounds: Vec<_> =
            (1..4).map(|i| doc.layer_pixel_bounds(i).unwrap()).collect();
        if !mode.distribution() {
            let anchors: Vec<_> = bounds
                .iter()
                .map(|r| {
                    let (a, b) = if mode.horizontal() {
                        (r.x0, r.x1)
                    } else {
                        (r.y0, r.y1)
                    };
                    a as f64 + (b - a) as f64 * mode.factor()
                })
                .collect();
            assert!(
                anchors.iter().all(|v| (v - anchors[0]).abs() <= 1.),
                "{mode:?} {anchors:?}"
            );
        }
    }
    let mut doc = shapes();
    let plan = doc
        .alignment_plan(
            2,
            Align::Right,
            Some(PixelRect { x0: 0, y0: 0, x1: 12, y1: 10 }),
        )
        .unwrap();
    assert!(doc.translate_layer_pixels(&plan));
    assert_eq!(doc.layer_pixel_bounds(1).unwrap().x1, 12);
}
#[test]
fn equal_gap_distribution_handles_different_widths_and_exact_boundary_anchors()
{
    let mut doc = shapes();
    let plan = doc.alignment_plan(14, Align::SpaceX, None).unwrap();
    doc.translate_layer_pixels(&plan);
    let a = doc.layer_pixel_bounds(1).unwrap();
    let b = doc.layer_pixel_bounds(2).unwrap();
    let c = doc.layer_pixel_bounds(3).unwrap();
    assert_eq!(a.x0, 0);
    assert_eq!(c.x1, 11);
    assert!(
        (b.x0 as i32 - a.x1 as i32 - (c.x0 as i32 - b.x1 as i32)).abs() <= 1
    );
}
#[test]
fn movement_translates_linked_mask_and_rejects_locked_invalid_or_duplicate_targets_atomically()
 {
    let mut doc = shapes();
    doc.create_layer_mask(1, 0);
    let id = doc.layers[1].id;
    assert!(doc.translate_layer_pixels(&[(id, [2, 0])]));
    assert_eq!(doc.layers[1].mask.as_ref().unwrap().coverage.value(12 + 2), 0);
    assert_eq!(doc.layers[1].mask.as_ref().unwrap().coverage.value(12), 255);
    let before = doc.clone();
    assert!(!doc.translate_layer_pixels(&[(id, [1, 0]), (id, [0, 1])]));
    assert_eq!(doc, before);
    doc.layers[1].locked = true;
    let before = doc.clone();
    assert!(doc.alignment_plan(14, Align::Left, None).is_err());
    assert!(!doc.translate_layer_pixels(&[(id, [1, 0])]));
    assert_eq!(doc, before);
    assert!(
        doc.plan_alignment_bounds(
            &[(id, PixelRect { x0: 5, x1: 1, y0: 0, y1: 1 })],
            Align::Left,
            None
        )
        .is_err()
    );
}

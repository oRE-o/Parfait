use super::*;
use crate::{LayerMask, MaskCoverage, PixelRect};

fn flat() -> Document {
    let mut document = Document::new(13, 7);
    document.add_layer();
    for (index, layer) in document.layers.iter_mut().enumerate() {
        layer.locked = false;
        layer.pixels = Arc::new(
            (0..91)
                .flat_map(|pixel| {
                    [
                        (pixel * 31 + index * 41) as u8,
                        (pixel * 17 + index * 73) as u8,
                        (pixel * 61 + index * 29) as u8,
                        (pixel * 19 + index * 83) as u8,
                    ]
                })
                .collect(),
        );
    }
    document
}

fn interpolate_reference(
    before: &[u8],
    after: &[u8],
    factors: impl Iterator<Item = f64>,
) -> Vec<u8> {
    before
        .as_chunks::<4>()
        .0
        .iter()
        .zip(after.as_chunks::<4>().0)
        .zip(factors)
        .flat_map(|((before, after), factor)| {
            let previous_alpha = f64::from(before[3]) / 255.;
            let current_alpha = f64::from(after[3]) / 255.;
            let alpha = previous_alpha * (1. - factor) + current_alpha * factor;
            if alpha == 0. {
                return [0; 4];
            }
            let mut result = [0; 4];
            for channel in 0..3 {
                result[channel] = ((f64::from(before[channel]) * previous_alpha * (1. - factor)
                    + f64::from(after[channel]) * current_alpha * factor)
                    / alpha)
                    .round()
                    .clamp(0., 255.) as u8;
            }
            result[3] = (alpha * 255.).round().clamp(0., 255.) as u8;
            result
        })
        .collect()
}

fn close(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len());
    for (offset, (actual, expected)) in actual.iter().zip(expected).enumerate() {
        assert!(
            actual.abs_diff(*expected) <= 1,
            "byte {offset}: {actual} != {expected}"
        );
    }
}

#[test]
#[ignore = "release CPU workload"]
fn through_composition_timing_workload() {
    use std::{hint::black_box, time::Instant};
    if cfg!(debug_assertions) {
        panic!("Run with --release");
    }
    for side in [256, 512] {
        let mut document = Document::new(side, side);
        for _ in 0..6 {
            document.add_layer();
        }
        for (index, layer) in document.layers.iter_mut().enumerate() {
            layer.locked = false;
            layer.blend_mode = if index.is_multiple_of(2) {
                BlendMode::Normal
            } else {
                BlendMode::Multiply
            };
            layer.pixels = Arc::new(
                (0..side * side)
                    .flat_map(|pixel| {
                        [
                            (pixel * 17) as u8,
                            (pixel * 31 + index as u32 * 7) as u8,
                            (pixel * 11) as u8,
                            (pixel * 23 + index as u32 * 47) as u8,
                        ]
                    })
                    .collect(),
            );
        }
        let flat = document.composite();
        document
            .group_layers_in_folder(0b11110000, "Inner".into())
            .unwrap();
        document.layers[8].blend_mode = BlendMode::Through;
        document
            .group_layers_in_folder((1 << 8) | 0b1111, "Outer".into())
            .unwrap();
        document.layers[9].blend_mode = BlendMode::Through;
        for attenuated in [false, true] {
            let expected = if attenuated {
                document.layers[9].opacity = 0.63;
                document.create_layer_mask(9, 193);
                interpolate_reference(
                    &vec![0; flat.len()],
                    &flat,
                    std::iter::repeat_n(0.63_f32 as f64 * 193. / 255., flat.len() / 4),
                )
            } else {
                flat.clone()
            };
            let mut elapsed = Vec::new();
            for _ in 0..25 {
                let started = Instant::now();
                let output = black_box(&document).composite();
                elapsed.push(started.elapsed().as_secs_f64() * 1000.);
                if attenuated {
                    close(&output, &expected);
                } else {
                    assert_eq!(output, expected);
                }
            }
            elapsed.sort_by(f64::total_cmp);
            eprintln!(
                "{side} through attenuated={attenuated}: p50_ms={:.4} p95_ms={:.4} row_scratch_bytes={}",
                elapsed[12],
                elapsed[23],
                side * 4 * 2
            );
        }
    }
}

#[test]
fn full_pass_through_matches_flat_blending_for_all_pixel_modes_and_shared_content() {
    for mode in BlendMode::ALL {
        let mut document = flat();
        document.layers[1].blend_mode = mode;
        document.layers[2].blend_mode = BlendMode::Overlay;
        document.layers[1].opacity = 0.63;
        document.layers[2].mask = Some(LayerMask {
            enabled: true,
            coverage: MaskCoverage::Uniform(173),
        });
        let before = document.clone();
        let expected = document.composite();
        document.group_layers_in_folder(6, "Pass".into()).unwrap();
        document.layers[3].blend_mode = BlendMode::Through;
        assert!(document.validate());
        assert_eq!(document.composite(), expected, "{mode:?}");
        for index in 0..3 {
            assert!(Arc::ptr_eq(
                &document.layers[index].pixels,
                &before.layers[index].pixels
            ));
        }
        let revision = document.revision;
        document.composite_region(PixelRect {
            x0: 2,
            y0: 1,
            x1: 11,
            y1: 6,
        });
        assert_eq!(document.revision, revision);
    }
}

#[test]
fn group_opacity_and_dense_masks_interpolate_once_in_premultiplied_space() {
    let mut document = flat();
    document.layers[1].blend_mode = BlendMode::Multiply;
    document.layers[2].blend_mode = BlendMode::Screen;
    let combined = document.composite();
    let mut backdrop = document.clone();
    backdrop.layers.truncate(1);
    backdrop.active = 0;
    let before = backdrop.composite();
    document.group_layers_in_folder(6, "Pass".into()).unwrap();
    document.layers[3].blend_mode = BlendMode::Through;
    for opacity in [0., 0.25, 0.5, 1.] {
        for coverage in [
            MaskCoverage::Uniform(0),
            MaskCoverage::Uniform(255),
            MaskCoverage::Uniform(127),
            MaskCoverage::Dense(Arc::new((0..91).map(|index| (index * 37) as u8).collect())),
        ] {
            document.layers[3].opacity = opacity;
            document.layers[3].mask = Some(LayerMask {
                enabled: true,
                coverage: coverage.clone(),
            });
            let expected = interpolate_reference(
                &before,
                &combined,
                (0..91).map(|index| f64::from(opacity) * f64::from(coverage.value(index)) / 255.),
            );
            let full = document.composite();
            close(&full, &expected);
            let rect = PixelRect {
                x0: 2,
                y0: 1,
                x1: 11,
                y1: 6,
            };
            let expected_region: Vec<_> = (1..6)
                .flat_map(|row| {
                    full[(row * 13 + 2) * 4..(row * 13 + 11) * 4]
                        .iter()
                        .copied()
                })
                .collect();
            assert_eq!(document.composite_region(rect), expected_region);
            document.layers[3].mask.as_mut().unwrap().enabled = false;
            close(
                &document.composite(),
                &interpolate_reference(
                    &before,
                    &combined,
                    std::iter::repeat_n(f64::from(opacity), 91),
                ),
            );
        }
    }
}

#[test]
fn nested_groups_scope_backdrops_and_apply_each_group_factor_once() {
    let mut document = flat();
    document.layers[1].blend_mode = BlendMode::Multiply;
    document.layers[2].blend_mode = BlendMode::Screen;
    let complete = document.composite();
    let mut prefix = document.clone();
    prefix.layers.truncate(2);
    prefix.active = 1;
    let prefix = prefix.composite();
    let mut base = document.clone();
    base.layers.truncate(1);
    base.active = 0;
    let base = base.composite();
    document.group_layers_in_folder(4, "Inner".into()).unwrap();
    document.layers[3].blend_mode = BlendMode::Through;
    document.layers[3].opacity = 0.5;
    document.layers[3].mask = Some(LayerMask {
        enabled: true,
        coverage: MaskCoverage::Uniform(127),
    });
    document.group_layers_in_folder(10, "Outer".into()).unwrap();
    document.layers[4].blend_mode = BlendMode::Through;
    document.layers[4].opacity = 0.3;
    let inner = interpolate_reference(
        &prefix,
        &complete,
        std::iter::repeat_n(0.5 * 127. / 255., 91),
    );
    close(
        &document.composite(),
        &interpolate_reference(&base, &inner, std::iter::repeat_n(f64::from(0.3f32), 91)),
    );
    document.layers[3].opacity = 1.;
    document.layers[3].mask = None;
    document.layers[4].blend_mode = BlendMode::Multiply;
    let expected = document.composite();
    assert!(document.ungroup_layer_folder(3));
    assert_eq!(document.composite(), expected);
}

#[test]
fn clipping_to_through_base_is_suppressed_and_clipped_through_is_isolated() {
    let mut document = flat();
    document.group_layers_in_folder(2, "Pass".into()).unwrap();
    document.layers[2].blend_mode = BlendMode::Through;
    document.layers[3].clipped = true;
    assert_eq!(document.clipping_base(3), None);
    assert_eq!(document.layer_clipping_base_below(3, 0), None);
    let expected = document.composite();
    document.layers[3].visible = false;
    assert_eq!(document.composite(), expected);
    document.layers[2].clipped = true;
    assert_eq!(document.clipping_base(2), Some(0));
    let through_clipped = document.composite();
    document.layers[2].blend_mode = BlendMode::Normal;
    assert_eq!(document.composite(), through_clipped);
    document.layers[1].blend_mode = BlendMode::Through;
    assert!(!document.validate());
}

#[test]
fn through_visibility_draft_filtering_and_hit_testing_keep_the_same_coverage_scope() {
    let mut document = flat();
    let mut backdrop = document.clone();
    backdrop.layers.truncate(1);
    backdrop.active = 0;
    let base = backdrop.composite();
    document.group_layers_in_folder(6, "Pass".into()).unwrap();
    document.layers[3].blend_mode = BlendMode::Through;
    document.layers[3].visible = false;
    assert_eq!(document.composite(), base);
    document.layers[3].visible = true;
    document.layers[3].draft = true;
    assert_eq!(document.composite_for_export(), base);
    document.layers[3].draft = false;
    document.layers[2].draft = true;
    let mut expected = flat();
    expected.layers[2].visible = false;
    assert_eq!(document.composite_for_export(), expected.composite());
    document.layers[3].mask = Some(LayerMask {
        enabled: true,
        coverage: MaskCoverage::Uniform(0),
    });
    assert_eq!(document.composite(), base);
    document.layers[0].pixels = Arc::new([20, 40, 60, 255].repeat(91));
    assert_eq!(document.layer_at(1, 1), Some(0));
    document.layers[3].mask = None;
    document.layers[1].pixels = Arc::new(vec![0; 91 * 4]);
    document.layers[2].visible = false;
    document.active = 0;
    document.add_layer();
    let added = document.active;
    document.move_layer_subtrees(1 << added, document.layers.len(), None);
    let added = document.active;
    document.layers[added].clipped = true;
    document.layers[added].pixels = Arc::new([200, 100, 50, 255].repeat(91));
    assert_eq!(document.clipping_base(added), None);
    assert_eq!(document.layer_at(1, 1), Some(0));
}

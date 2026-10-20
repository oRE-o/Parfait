//! Reproducible CPU edit + composite workload; never a pen/display latency test.
use parfait_core::*;
use std::{hint::black_box, sync::Arc, time::Instant};
fn fixture(w: u32, h: u32) -> Document {
    let mut d = Document::new_with_paper(w, h);
    while d.layers.len() < 8 {
        d.add_layer();
    }
    for (n, l) in d.layers.iter_mut().enumerate().skip(1) {
        let mut p = vec![0; (w * h * 4) as usize];
        for y in 0..h {
            for x in 0..w {
                let i = ((y * w + x) * 4) as usize;
                p[i..i + 4].copy_from_slice(&[
                    (x.wrapping_mul(17) + n as u32 * 31) as u8,
                    (y * 13) as u8,
                    (x + y * 3) as u8,
                    if (x / 32 + y / 24 + n as u32).is_multiple_of(3) {
                        0
                    } else {
                        160
                    },
                ]);
            }
        }
        l.pixels = Arc::new(p);
        l.opacity = 0.6;
    }
    d.create_layer_mask(4, 180);
    d.set_selection(Some(MaskCoverage::Dense(Arc::new(
        (0..w * h)
            .map(|i| if i % w > w / 4 && i / w > h / 4 { 200 } else { 0 })
            .collect(),
    ))));
    assert!(d.validate());
    d
}
fn measure(d: &Document, label: &str, edit: impl Fn(&mut Document)) {
    let mut times = Vec::new();
    let mut digest = 0_u64;
    for _ in 0..9 {
        let mut next = d.clone();
        let start = Instant::now();
        edit(&mut next);
        let pixels = black_box(next.composite());
        times.push(start.elapsed().as_secs_f64() * 1000.);
        assert!(next.validate());
        let checksum = pixels.iter().fold(14695981039346656037_u64, |h, b| {
            (h ^ u64::from(*b)).wrapping_mul(1099511628211)
        });
        if digest != 0 {
            assert_eq!(digest, checksum);
        }
        digest = checksum;
        let rect = PixelRect { x0: 11, y0: 13, x1: 55, y1: 47 };
        let region = next.composite_region(rect);
        for y in rect.y0..rect.y1 {
            let i = (y * next.width + rect.x0) as usize * 4;
            let r = (y - rect.y0) as usize * (rect.x1 - rect.x0) as usize * 4;
            assert_eq!(
                &pixels[i..i + (rect.x1 - rect.x0) as usize * 4],
                &region[r..r + (rect.x1 - rect.x0) as usize * 4]
            );
        }
    }
    times.sort_by(f64::total_cmp);
    println!(
        "{},{},{},{:.3},{:.3},{digest:016x}",
        d.width, d.height, label, times[4], times[8]
    );
}
fn main() {
    println!("width,height,edit_plus_composite,median_ms,p95_ms,rgba_fnv1a");
    for (w, h) in [(512, 384), (1024, 768)] {
        let d = fixture(w, h);
        // Exact permutation check includes opacity, masks and selection storage.
        let original = d.composite();
        let mut rotated = d.clone();
        assert!(rotated.edit_canvas(CanvasEdit::RotateRight));
        let result = rotated.composite();
        for y in 0..h {
            for x in 0..w {
                let src = ((y * w + x) * 4) as usize;
                let dest = ((x * h + h - 1 - y) * 4) as usize;
                assert_eq!(&original[src..src + 4], &result[dest..dest + 4]);
            }
        }
        measure(&d, "brightness_contrast", |d| {
            assert!(d.edit_layer_pixels(
                d.active,
                PixelEdit::BrightnessContrast {
                    brightness: 0.1,
                    contrast: 0.15
                }
            ));
        });
        measure(&d, "rotate_right", |d| {
            assert!(d.edit_canvas(CanvasEdit::RotateRight));
        });
        measure(&d, "channel_levels", |d| {
            let mut levels = [ToneLevels::default(); 4];
            levels[0].black = 12;
            levels[0].white = 240;
            levels[0].output_black = 8;
            levels[1].gamma = 1.3;
            assert!(
                d.adjust_layer_tones(0b110, &ToneAdjustment::Levels(levels))
            );
        });
        measure(&d, "hsv_preview_edit", |d| {
            assert!(d.adjust_layer_tones(
                2,
                &ToneAdjustment::Hsv { hue: 70., saturation: 0.45, value: 0.1 }
            ));
        });
        measure(&d, "smooth_channel_curves", |d| {
            let mut curves = std::array::from_fn(|_| ResponseCurve::default());
            curves[0].points =
                vec![[0., 0.], [0.25, 0.16], [0.72, 0.84], [1., 1.]];
            assert!(
                d.adjust_layer_tones(0b110, &ToneAdjustment::Curves(curves))
            );
        });
        measure(&d, "multistop_gradient", |d| {
            assert!(d.adjust_layer_tones(
                2,
                &ToneAdjustment::Gradient {
                    stops: vec![
                        GradientStop {
                            position: 0.,
                            color: [25, 12, 61],
                            midpoint: 0.45
                        },
                        GradientStop {
                            position: 0.48,
                            color: [180, 72, 94],
                            midpoint: 0.6
                        },
                        GradientStop {
                            position: 1.,
                            color: [255, 234, 185],
                            midpoint: 0.5
                        }
                    ],
                    reverse: false,
                    linear_light: true
                }
            ));
        });
        measure(&d, "silhouette_shading", |d| {
            assert!(d.add_local_shading(
                d.active,
                LocalShading { radius: 64., ..Default::default() }
            ));
        });
        measure(&d, "resize_125pct", |d| {
            assert!(d.edit_canvas(CanvasEdit::Resize {
                width: w * 5 / 4,
                height: h * 5 / 4
            }));
        });
        measure(&d, "projective", |d| {
            let t = RasterTransform {
                angle: 12.,
                perspective: [0.08, -0.04],
                ..Default::default()
            };
            assert!(d.transform_layer_pixels(d.active, &t));
        });
        measure(&d, "mesh_4x5", |d| {
            let mut t =
                RasterTransform { kind: WarpKind::Mesh, ..Default::default() };
            t.resize_grid(4, 5);
            t.nodes[9] = [0.04, 0.03];
            assert!(d.transform_layer_pixels(d.active, &t));
        });
        measure(&d, "puppet_3pins", |d| {
            let t = RasterTransform {
                kind: WarpKind::Pins,
                pins: vec![
                    WarpPin {
                        source: [0.2, 0.3],
                        target: [0.25, 0.32],
                        rotation: 0.,
                    },
                    WarpPin {
                        source: [0.75, 0.7],
                        target: [0.75, 0.7],
                        rotation: 0.,
                    },
                    WarpPin {
                        source: [0.5, 0.9],
                        target: [0.5, 0.9],
                        rotation: 0.,
                    },
                ],
                ..Default::default()
            };
            assert!(d.transform_layer_pixels(d.active, &t));
        });
        let mut native = d.clone();
        native
            .create_content_layer(
                LayerContent::Vector(VectorContent {
                    matrix: Affine::translation(70., 30.),
                    paths: vec![VectorPath {
                        points: Arc::new(vec![
                            [0., 0.],
                            [140., 60.],
                            [240., 0.],
                        ]),
                        color: [20, 100, 200, 210],
                        width: 12.,
                        closed: false,
                        fill: false,
                    }],
                }),
                "V".into(),
            )
            .unwrap();
        native.create_content_layer(LayerContent::Text(TextContent{text:"Parfait · 편집 검사".into(),font:Arc::new(include_bytes!("../../../apps/parfait/assets/pretendard/Pretendard-Regular.otf").to_vec()),size:38.,color:[30,20,60,255],matrix:Affine::translation(55.,100.)}),"T".into()).unwrap();
        let expected = native.composite();
        let mut raster = native.clone();
        while let Some(n) = raster.contents.first() {
            let index = raster.layer_index(n.layer).unwrap();
            assert!(raster.rasterize_content(index));
        }
        assert_eq!(expected, raster.composite());
        measure(&native, "native_canvas_resize", |d| {
            assert!(d.edit_canvas(CanvasEdit::Resize {
                width: w * 5 / 4,
                height: h * 5 / 4
            }));
        });
    }
    println!(
        "pixel_equivalence=exact_rotation_and_native_rasterization_and_region_checks"
    );
}

use super::*;

fn reference(document: &Document, rect: PixelRect, mask: u64) -> Vec<u8> {
    let bounds = rect.clipped(document.width, document.height);
    let mut output = Vec::new();
    for row in bounds.y0..bounds.y1 {
        for column in bounds.x0..bounds.x1 {
            let offset = ((row * document.width + column) * 4) as usize;
            let mut backdrop = [0; 4];
            for (index, base) in document.layers.iter().enumerate() {
                if base.clipped
                    || !base.visible
                    || base.opacity == 0.
                    || mask & (1 << index) == 0
                    || base.pixels[offset + 3] == 0
                {
                    continue;
                }
                let source = &base.pixels[offset..offset + 4];
                let mut clipped_color = None;
                for (clip_index, clip) in
                    document.layers.iter().enumerate().skip(index + 1)
                {
                    if !clip.clipped {
                        break;
                    }
                    if !clip.visible
                        || clip.opacity == 0.
                        || mask & (1 << clip_index) == 0
                        || clip.pixels[offset + 3] == 0
                    {
                        continue;
                    }
                    let color = clipped_color.get_or_insert_with(|| {
                        let backdrop_alpha = backdrop[3] as f32 / 255.;
                        let mut color = [0, 0, 0, 255];
                        for channel in 0..3 {
                            let source = source[channel] as f32 / 255.;
                            let blended = base.blend_mode.channel(
                                backdrop[channel] as f32 / 255.,
                                source,
                            );
                            color[channel] = ((source * (1. - backdrop_alpha)
                                + blended * backdrop_alpha)
                                * 255.)
                                .round()
                                .clamp(0., 255.)
                                as u8;
                        }
                        color
                    });
                    let clip_pixel = &clip.pixels[offset..offset + 4];
                    composite_blend(
                        color,
                        [clip_pixel[0], clip_pixel[1], clip_pixel[2]],
                        clip_pixel[3] as f32 / 255. * clip.opacity,
                        clip.blend_mode,
                    );
                }
                let (color, mode) = if let Some(color) = clipped_color {
                    ([color[0], color[1], color[2]], BlendMode::Normal)
                } else {
                    ([source[0], source[1], source[2]], base.blend_mode)
                };
                composite_blend(
                    &mut backdrop,
                    color,
                    source[3] as f32 / 255. * base.opacity,
                    mode,
                );
            }
            output.extend_from_slice(&backdrop);
        }
    }
    output
}

fn next(seed: &mut u32) -> u32 {
    *seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
    *seed
}

fn fixture(width: u32, height: u32, count: usize, seed: &mut u32) -> Document {
    let mut document = Document::new(width, height);
    while document.layers.len() < count {
        document.add_layer();
    }
    for (index, layer) in document.layers.iter_mut().enumerate() {
        let value = next(seed);
        layer.clipped = index % 4 != 0;
        layer.visible = value & 3 != 0;
        layer.opacity = [
            0.,
            0.49,
            1.,
            0.7,
            1. / 255.,
            f32::MIN_POSITIVE,
            f32::from_bits(1),
        ][index % 7];
        layer.blend_mode =
            BlendMode::ALL[value as usize % BlendMode::ALL.len()];
        for pixel in Arc::make_mut(&mut layer.pixels).as_chunks_mut::<4>().0 {
            *pixel = next(seed).to_le_bytes();
        }
    }
    document
}

fn verify(document: &Document, mask: u64) {
    for rect in [
        PixelRect { x0: 0, y0: 0, x1: document.width, y1: document.height },
        PixelRect {
            x0: document.width / 3,
            y0: document.height / 3,
            x1: document.width + 9,
            y1: document.height + 1,
        },
        PixelRect { x0: document.width + 1, y0: 0, x1: 0, y1: document.height },
    ] {
        assert_eq!(
            document.composite_region_layers(rect, mask),
            reference(document, rect, mask),
            "{mask:x} {rect:?}"
        );
    }
}

#[test]
fn row_compositor_matches_pixel_reference_for_all_base_and_clip_mode_pairs() {
    let mut seed = 5;
    for _ in 0..4 {
        let mut document = fixture(11, 7, 8, &mut seed);
        document.layers[0].visible = true;
        document.layers[0].opacity = 0.49;
        for base_mode in BlendMode::ALL {
            for clip_mode in BlendMode::ALL {
                for layer in &mut document.layers {
                    layer.blend_mode =
                        if layer.clipped { clip_mode } else { base_mode };
                }
                let before = document.clone();
                for mask in [u64::MAX, 0, 1, 0b10101010, 0b11101111] {
                    verify(&document, mask);
                }
                assert_eq!(document, before);
                assert!(document.same_edit_state(&before));
            }
        }
    }
}

#[test]
fn row_compositor_matches_reference_at_maximum_width_and_layer_boundaries() {
    let mut seed = 19;
    for (width, height) in [(1, 1), (1, 23), (4096, 2), (17, 13)] {
        let mut document = fixture(width, height, 64, &mut seed);
        for scenario in 0..5 {
            for (index, layer) in document.layers.iter_mut().enumerate() {
                layer.clipped = match scenario {
                    0 => false,
                    1 => true,
                    2 => index > 0,
                    _ => index % 4 != 0,
                };
                if scenario == 3 {
                    layer.visible = !layer.clipped;
                }
                if scenario == 4 {
                    layer.visible = layer.clipped;
                }
            }
            for mask in [
                u64::MAX,
                0,
                1,
                1 << 63,
                0xAAAAAAAAAAAAAAAA,
                0x7FFFFFFFFFFFFFFF,
            ] {
                verify(&document, mask);
            }
        }
    }
}

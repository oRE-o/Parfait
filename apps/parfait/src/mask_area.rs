use parfait_core::{MaskCoverage, PixelRect};

const TINT: [u8; 3] = [112, 96, 255];

fn tint_pixel(pixel: &mut [u8], alpha: u16) {
    if alpha == 0 {
        return;
    }
    for (target, color) in pixel[..3].iter_mut().zip(TINT) {
        *target = ((u16::from(*target) * (255 - alpha)
            + u16::from(color) * alpha
            + 127)
            / 255) as u8;
    }
}

pub(crate) fn tint_mask_area(
    pixels: &mut [u8],
    dimensions: [u32; 2],
    coverage: &MaskCoverage,
    region: PixelRect,
) -> bool {
    let [width, height] = dimensions;
    if width == 0
        || height == 0
        || width > 4096
        || height > 4096
        || region != region.clipped(width, height)
        || region.x0 >= region.x1
        || region.y0 >= region.y1
    {
        return false;
    }
    let region_width = (region.x1 - region.x0) as usize;
    if pixels.len() != region_width * (region.y1 - region.y0) as usize * 4 {
        return false;
    }
    match coverage {
        MaskCoverage::Uniform(value) => {
            let alpha = (u16::from(255 - value) * 128 + 127) / 255;
            if alpha != 0 {
                for pixel in pixels.as_chunks_mut::<4>().0.iter_mut() {
                    tint_pixel(pixel, alpha);
                }
            }
        }
        MaskCoverage::Dense(values) => {
            if values.len() != width as usize * height as usize {
                return false;
            }
            for (row_index, row) in
                pixels.chunks_exact_mut(region_width * 4).enumerate()
            {
                let offset = (row_index + region.y0 as usize) * width as usize
                    + region.x0 as usize;
                for (column, pixel) in
                    row.as_chunks_mut::<4>().0.iter_mut().enumerate()
                {
                    let alpha =
                        (u16::from(255 - values[offset + column]) * 128 + 127)
                            / 255;
                    tint_pixel(pixel, alpha);
                }
            }
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn tint_matches_independent_integer_reference_and_preserves_alpha() {
        let region = PixelRect { x0: 0, y0: 0, x1: 256, y1: 1 };
        for coverage in 0..=255 {
            let original: Vec<u8> = (0..=255)
                .flat_map(|value| [value, 255 - value, value / 2, value])
                .collect();
            let mut current = original.clone();
            assert!(tint_mask_area(
                &mut current,
                [256, 1],
                &MaskCoverage::Uniform(coverage),
                region
            ));
            let alpha = ((255 - u32::from(coverage)) * 128 + 127) / 255;
            for (before, after) in original
                .as_chunks::<4>()
                .0
                .iter()
                .zip(current.as_chunks::<4>().0.iter())
            {
                assert_eq!(after[3], before[3]);
                for channel in 0..3 {
                    let expected = (u32::from(before[channel]) * (255 - alpha)
                        + u32::from(TINT[channel]) * alpha
                        + 127)
                        / 255;
                    assert_eq!(u32::from(after[channel]), expected);
                }
            }
            let mut dense = original;
            assert!(tint_mask_area(
                &mut dense,
                [256, 1],
                &MaskCoverage::Dense(Arc::new(vec![coverage; 256])),
                region
            ));
            assert_eq!(current, dense);
        }
    }

    #[test]
    fn dense_partial_tint_matches_full_image_at_nonzero_origin() {
        let coverage = MaskCoverage::Dense(Arc::new(
            (0..35).map(|index| (index * 31) as u8).collect(),
        ));
        let original: Vec<u8> = (0..35)
            .flat_map(|index| [index as u8 * 7, 120, 240, 255])
            .collect();
        let mut full = original.clone();
        assert!(tint_mask_area(
            &mut full,
            [7, 5],
            &coverage,
            PixelRect { x0: 0, y0: 0, x1: 7, y1: 5 }
        ));
        let region = PixelRect { x0: 2, y0: 1, x1: 6, y1: 4 };
        let mut partial = Vec::new();
        let mut expected = Vec::new();
        for row in 1..4 {
            let start = (row * 7 + 2) * 4;
            partial.extend_from_slice(&original[start..start + 16]);
            expected.extend_from_slice(&full[start..start + 16]);
        }
        assert!(tint_mask_area(&mut partial, [7, 5], &coverage, region));
        assert_eq!(partial, expected);
    }

    #[test]
    fn invalid_tint_input_is_rejected_without_writes() {
        for case in 0..5 {
            let mut pixels = vec![180; 16];
            let before = pixels.clone();
            let mut region = PixelRect { x0: 0, y0: 0, x1: 2, y1: 2 };
            let mut dimensions = [2, 2];
            let mut mask = MaskCoverage::Uniform(0);
            match case {
                0 => region.x1 = 3,
                1 => dimensions[0] = 0,
                2 => region.x0 = region.x1,
                3 => mask = MaskCoverage::Dense(Arc::new(vec![0; 3])),
                _ => region.y1 = 1,
            }
            assert!(!tint_mask_area(&mut pixels, dimensions, &mask, region));
            assert_eq!(pixels, before);
        }
    }
}

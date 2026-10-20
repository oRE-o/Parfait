//! Separable blend functions on stored (sRGB-encoded) RGB, with straight-alpha source-over.
use serde::{Deserialize, Serialize};

#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize,
)]
pub enum BlendMode {
    #[default]
    Normal,
    Multiply,
    Screen,
    Overlay,
    Darken,
    Lighten,
    ColorDodge,
    ColorBurn,
    HardLight,
    SoftLight,
    Difference,
    Exclusion,
    Add,
    Subtract,
    Through,
}
impl BlendMode {
    pub const ALL: [Self; 14] = [
        Self::Normal,
        Self::Multiply,
        Self::Screen,
        Self::Overlay,
        Self::Darken,
        Self::Lighten,
        Self::ColorDodge,
        Self::ColorBurn,
        Self::HardLight,
        Self::SoftLight,
        Self::Difference,
        Self::Exclusion,
        Self::Add,
        Self::Subtract,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Normal => "Normal",
            Self::Multiply => "Multiply",
            Self::Screen => "Screen",
            Self::Overlay => "Overlay",
            Self::Darken => "Darken",
            Self::Lighten => "Lighten",
            Self::ColorDodge => "Color dodge",
            Self::ColorBurn => "Color burn",
            Self::HardLight => "Hard light",
            Self::SoftLight => "Soft light",
            Self::Difference => "Difference",
            Self::Exclusion => "Exclusion",
            Self::Add => "Add",
            Self::Subtract => "Subtract",
            Self::Through => "Through",
        }
    }
    pub(crate) fn channel(self, b: f32, s: f32) -> f32 {
        let hard = |b: f32, s: f32| {
            if s <= 0.5 { 2. * b * s } else { 1. - 2. * (1. - b) * (1. - s) }
        };
        match self {
            Self::Normal | Self::Through => s,
            Self::Multiply => b * s,
            Self::Screen => b + s - b * s,
            Self::Overlay => hard(s, b),
            Self::Darken => b.min(s),
            Self::Lighten => b.max(s),
            Self::ColorDodge => {
                if b == 0. {
                    0.
                } else if s == 1. {
                    1.
                } else {
                    (b / (1. - s)).min(1.)
                }
            }
            Self::ColorBurn => {
                if b == 1. {
                    1.
                } else if s == 0. {
                    0.
                } else {
                    1. - ((1. - b) / s).min(1.)
                }
            }
            Self::HardLight => hard(b, s),
            Self::SoftLight => {
                if s <= 0.5 {
                    b - (1. - 2. * s) * b * (1. - b)
                } else {
                    let d = if b <= 0.25 {
                        ((16. * b - 12.) * b + 4.) * b
                    } else {
                        b.sqrt()
                    };
                    b + (2. * s - 1.) * (d - b)
                }
            }
            Self::Difference => (b - s).abs(),
            Self::Exclusion => b + s - 2. * b * s,
            Self::Add => (b + s).min(1.),
            Self::Subtract => (b - s).max(0.),
        }
    }
}
pub(crate) fn composite_blend(
    dst: &mut [u8],
    rgb: [u8; 3],
    alpha: f32,
    mode: BlendMode,
) {
    if mode == BlendMode::Normal || dst[3] == 0 {
        crate::composite_over(dst, rgb, alpha);
        return;
    }
    let sa = alpha.clamp(0., 1.);
    if sa == 0. {
        return;
    }
    let da = dst[3] as f32 / 255.;
    let a = sa + da * (1. - sa);
    for i in 0..3 {
        let b = dst[i] as f32 / 255.;
        let s = rgb[i] as f32 / 255.;
        let c = sa * (1. - da) * s
            + sa * da * mode.channel(b, s)
            + (1. - sa) * da * b;
        dst[i] = (c / a * 255.).round().clamp(0., 255.) as u8;
    }
    dst[3] = (a * 255.).round() as u8;
}
pub(crate) fn composite_opaque(
    dst: &mut [u8; 4],
    rgb: [u8; 3],
    alpha: f32,
    mode: BlendMode,
) {
    debug_assert_eq!(dst[3], 255);
    let source_alpha = alpha.clamp(0., 1.);
    if source_alpha == 0. {
        return;
    }
    if mode == BlendMode::Normal {
        if source_alpha == 1. {
            dst[..3].copy_from_slice(&rgb);
            return;
        }
        for channel in 0..3 {
            dst[channel] = (rgb[channel] as f32 * source_alpha
                + dst[channel] as f32 * (1. - source_alpha))
                .round() as u8;
        }
    } else {
        for channel in 0..3 {
            let backdrop = dst[channel] as f32 / 255.;
            let source = rgb[channel] as f32 / 255.;
            let color = source_alpha * mode.channel(backdrop, source)
                + (1. - source_alpha) * backdrop;
            dst[channel] = (color * 255.).round().clamp(0., 255.) as u8;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn known_channels_and_alpha_edges() {
        let expected = [
            0.75, 0.1875, 0.8125, 0.375, 0.25, 0.75, 1., 0., 0.625, 0.375, 0.5,
            0.625, 1., 0.,
        ];
        for (mode, expected) in BlendMode::ALL.into_iter().zip(expected) {
            assert!(
                (mode.channel(0.25, 0.75) - expected).abs() < 0.00001,
                "{mode:?}"
            );
            let mut empty = [23, 99, 11, 0];
            composite_blend(&mut empty, [51, 102, 153], 0.5, mode);
            assert_eq!(empty, [51, 102, 153, 128]);
            let mut unchanged = [20, 40, 60, 128];
            composite_blend(&mut unchanged, [255; 3], 0., mode);
            assert_eq!(unchanged, [20, 40, 60, 128]);
        }
        let mut dst = [128, 128, 128, 128];
        composite_blend(&mut dst, [128; 3], 0.5, BlendMode::Multiply);
        assert_eq!(dst, [107, 107, 107, 192]);
        assert_eq!(BlendMode::ColorDodge.channel(0., 1.), 0.);
        assert_eq!(BlendMode::ColorBurn.channel(1., 0.), 1.);
    }

    #[test]
    fn opaque_specialization_matches_general_blending_for_byte_pairs_and_opacity_edges()
     {
        for mode in BlendMode::ALL {
            for backdrop in 0..=255u8 {
                for source in 0..=255u8 {
                    for alpha in [
                        0.,
                        1. / 255.,
                        0.5,
                        127. / 255.,
                        128. / 255.,
                        254. / 255.,
                        1.,
                        0.7,
                    ] {
                        let mut expected =
                            [backdrop, source, 255 - backdrop, 255];
                        let mut actual = expected;
                        let rgb = [source, 255 - backdrop, backdrop];
                        composite_blend(&mut expected, rgb, alpha, mode);
                        composite_opaque(&mut actual, rgb, alpha, mode);
                        assert_eq!(
                            actual, expected,
                            "{mode:?} {backdrop} {source} {alpha}"
                        );
                    }
                }
            }
        }
        let mut seed = 7u32;
        for mode in BlendMode::ALL {
            for _ in 0..20_000 {
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                let mut actual = [
                    (seed >> 24) as u8,
                    (seed >> 16) as u8,
                    (seed >> 8) as u8,
                    255,
                ];
                let mut expected = actual;
                let rgb = [seed as u8, (seed >> 9) as u8, (seed >> 17) as u8];
                let alpha = (seed & 0xFFFFFF) as f32 / 0xFFFFFF as f32;
                composite_blend(&mut expected, rgb, alpha, mode);
                composite_opaque(&mut actual, rgb, alpha, mode);
                assert_eq!(actual, expected, "{mode:?} {seed} {alpha}");
            }
        }
    }
}

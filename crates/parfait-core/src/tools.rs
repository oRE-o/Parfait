//! Bounded region selection and raster tool operations shared by desktop inputs.
use crate::{Document, LayerKind, MaskCoverage, SelectionMode, composite_over};
use std::sync::Arc;
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SampleSource {
    Composite,
    Active,
    Reference,
}
impl Document {
    pub fn region_at(
        &self,
        x: u32,
        y: u32,
        tolerance: f32,
        contiguous: bool,
        source: SampleSource,
    ) -> Option<MaskCoverage> {
        if !self.validate()
            || x >= self.width
            || y >= self.height
            || !tolerance.is_finite()
        {
            return None;
        }
        let image;
        let pixels: &[u8] = match source {
            SampleSource::Active => {
                image = (0..self.width * self.height)
                    .flat_map(|i| {
                        self.layers[self.active].source_pixel(i as usize)
                    })
                    .collect::<Vec<_>>();
                &image
            }
            SampleSource::Composite => {
                image = self.composite();
                &image
            }
            SampleSource::Reference => {
                let mut doc = self.clone();
                let refs: Vec<_> = doc
                    .layers
                    .iter()
                    .enumerate()
                    .filter(|(_, l)| l.reference)
                    .map(|(i, _)| i)
                    .collect();
                if refs.is_empty() {
                    return None;
                }
                for i in 0..doc.layers.len() {
                    if doc.layers[i].kind != LayerKind::Folder {
                        doc.layers[i].visible &= refs.iter().any(|r| {
                            *r == i
                                || doc.layer_subtree_mask(*r).is_some_and(
                                    |mask| mask & (1_u64 << i) != 0,
                                )
                        });
                    }
                }
                image = doc.composite();
                &image
            }
        };
        let count = (self.width * self.height) as usize;
        let seed = (y * self.width + x) as usize;
        let target = &pixels[seed * 4..seed * 4 + 4];
        let limit = (tolerance.clamp(0., 100.) * 2.55).round() as i16;
        let matches = |i: usize| {
            let p = &pixels[i * 4..i * 4 + 4];
            if target[3] == 0 && p[3] == 0 {
                return true;
            }
            p.iter()
                .zip(target)
                .all(|(a, b)| (i16::from(*a) - i16::from(*b)).abs() <= limit)
        };
        let mut values = vec![0; count];
        if contiguous {
            let width = self.width as usize;
            let mut stack = vec![seed];
            while let Some(i) = stack.pop() {
                if values[i] != 0 || !matches(i) {
                    continue;
                }
                let row = i / width;
                let mut left = i;
                let mut right = i;
                while left > row * width
                    && values[left - 1] == 0
                    && matches(left - 1)
                {
                    left -= 1;
                }
                while right + 1 < (row + 1) * width
                    && values[right + 1] == 0
                    && matches(right + 1)
                {
                    right += 1;
                }
                values[left..=right].fill(255);
                for next_row in [
                    row.checked_sub(1),
                    (row + 1 < self.height as usize).then_some(row + 1),
                ]
                .into_iter()
                .flatten()
                {
                    let mut run = false;
                    for x in left % width..=right % width {
                        let next = next_row * width + x;
                        let open = values[next] == 0 && matches(next);
                        if open && !run {
                            stack.push(next);
                        }
                        run = open;
                    }
                }
            }
        } else {
            for (i, v) in values.iter_mut().enumerate() {
                if matches(i) {
                    *v = 255;
                }
            }
        }
        Some(if values.iter().all(|v| *v == 255) {
            MaskCoverage::Uniform(255)
        } else {
            MaskCoverage::Dense(Arc::new(values))
        })
    }
    pub fn combine_selection(
        &mut self,
        source: MaskCoverage,
        mode: SelectionMode,
    ) -> bool {
        if !self.validate() {
            return false;
        }
        let n = (self.width * self.height) as usize;
        if matches!(&source,MaskCoverage::Dense(v) if v.len()!=n) {
            return false;
        }
        let coverage = match (mode, self.selection.as_ref()) {
            (SelectionMode::Replace, _) | (_, None) => source,
            (_, Some(before)) => {
                let mut values = Vec::with_capacity(n);
                for i in 0..n {
                    values.push(mode.combine(before.value(i), source.value(i)));
                }
                MaskCoverage::Dense(Arc::new(values))
            }
        };
        let coverage = match coverage {
            MaskCoverage::Dense(values)
                if values.iter().all(|v| *v == values[0]) =>
            {
                MaskCoverage::Uniform(values[0])
            }
            other => other,
        };
        self.set_selection(if matches!(coverage, MaskCoverage::Uniform(0)) {
            None
        } else {
            Some(coverage)
        })
    }
    /// Draw into a raster target, respecting both selection and transparent-pixel lock.
    pub fn paint_tool(
        &mut self,
        coverage: Option<&MaskCoverage>,
        color: [u8; 3],
        end_color: Option<[u8; 3]>,
        gradient: Option<([f32; 2], [f32; 2], bool)>,
        opacity: f32,
        erase: bool,
    ) -> bool {
        if !self.validate()
            || !opacity.is_finite()
            || !(0. ..=1.).contains(&opacity)
            || !self.layer_ancestors_unlocked(self.active)
            || self.layers[self.active].kind != LayerKind::Raster
            || self.native_content(self.active).is_some()
        {
            return false;
        }
        if gradient.is_some_and(|(s, e, _)| {
            s.into_iter().chain(e).any(|n| !n.is_finite())
                || (s[0] - e[0]).powi(2) + (s[1] - e[1]).powi(2) < 0.01
        }) {
            return false;
        }
        let n = (self.width * self.height) as usize;
        if matches!(coverage,Some(MaskCoverage::Dense(v)) if v.len()!=n) {
            return false;
        }
        let alpha_locked = self.layers[self.active].alpha_locked;
        let mut output: Option<Vec<u8>> = None;
        for i in 0..n {
            let mut amount = opacity
                * coverage.map_or(1., |c| f32::from(c.value(i)) / 255.)
                * self.selection_value(i) as f32
                / 255.;
            if amount == 0. {
                continue;
            }
            let mut rgb = color;
            if let Some((start, end, radial)) = gradient {
                let delta = [end[0] - start[0], end[1] - start[1]];
                let p = [
                    (i % self.width as usize) as f32 + 0.5 - start[0],
                    (i / self.width as usize) as f32 + 0.5 - start[1],
                ];
                let d = delta[0] * delta[0] + delta[1] * delta[1];
                if d < 0.01 {
                    return false;
                }
                let t = if radial {
                    ((p[0] * p[0] + p[1] * p[1]) / d).sqrt()
                } else {
                    (p[0] * delta[0] + p[1] * delta[1]) / d
                }
                .clamp(0., 1.);
                if let Some(end) = end_color {
                    for c in 0..3 {
                        rgb[c] = (f32::from(color[c]) * (1. - t)
                            + f32::from(end[c]) * t)
                            .round() as u8;
                    }
                } else {
                    amount *= 1. - t;
                }
            }
            let old: &[u8] = &self.layers[self.active].pixels[i * 4..i * 4 + 4];
            let mut pixel: [u8; 4] = old.try_into().unwrap();
            if alpha_locked {
                if !erase {
                    crate::brush::recolor_locked(&mut pixel, rgb, amount);
                }
            } else if erase {
                pixel[3] = (f32::from(pixel[3]) * (1. - amount)).round() as u8;
                if pixel[3] == 0 {
                    pixel = [0; 4];
                }
            } else {
                composite_over(&mut pixel, rgb, amount);
            }
            if pixel.as_slice() != old {
                output.get_or_insert_with(|| {
                    self.layers[self.active].pixels.as_ref().clone()
                })[i * 4..i * 4 + 4]
                    .copy_from_slice(&pixel);
            }
        }
        if let Some(output) = output {
            self.layers[self.active].pixels = Arc::new(output);
            self.touch();
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn regions_distinguish_contiguous_global_and_reference_sources() {
        let mut doc = Document::new(5, 2);
        let pixels = Arc::make_mut(&mut doc.layers[1].pixels);
        pixels
            .as_chunks_mut::<4>()
            .0
            .iter_mut()
            .for_each(|p| p.copy_from_slice(&[0, 0, 0, 255]));
        for i in [0, 1, 4, 5, 6, 9] {
            pixels[i * 4..i * 4 + 4].copy_from_slice(&[255, 0, 0, 255]);
        }
        let local =
            doc.region_at(0, 0, 0., true, SampleSource::Active).unwrap();
        let global =
            doc.region_at(0, 0, 0., false, SampleSource::Active).unwrap();
        assert_eq!((0..10).filter(|i| local.value(*i) > 0).count(), 4);
        assert_eq!((0..10).filter(|i| global.value(*i) > 0).count(), 6);
        assert!(
            doc.region_at(0, 0, 0., true, SampleSource::Reference).is_none()
        );
        doc.layers[1].reference = true;
        assert_eq!(
            doc.region_at(0, 0, 0., true, SampleSource::Reference).unwrap(),
            local
        );
    }
    #[test]
    fn fill_gradient_erase_obey_selection_alpha_lock_and_undo() {
        let mut doc = Document::new(4, 1);
        doc.set_selection(Some(MaskCoverage::Dense(Arc::new(vec![
            0, 128, 255, 255,
        ]))));
        let mut history = crate::History::default();
        history.begin(&doc, "Fill");
        assert!(doc.paint_tool(None, [255, 0, 0], None, None, 1., false));
        assert!(history.commit(&doc));
        assert_eq!(doc.layers[1].pixels[3], 0);
        assert_eq!(doc.layers[1].pixels[7], 128);
        assert_eq!(doc.layers[1].pixels[11], 255);
        let alpha = doc.layers[1].pixels[7];
        doc.layers[1].alpha_locked = true;
        assert!(doc.paint_tool(
            None,
            [0, 0, 255],
            Some([0, 255, 0]),
            Some(([0.5, 0.], [3.5, 0.], false)),
            1.,
            false
        ));
        assert_eq!(doc.layers[1].pixels[7], alpha);
        let before = doc.clone();
        assert!(!doc.paint_tool(None, [0; 3], None, None, 1., true));
        assert!(doc.same_edit_state(&before));
        doc.layers[1].alpha_locked = false;
        assert!(doc.paint_tool(None, [0; 3], None, None, 1., true));
        assert_eq!(doc.layers[1].pixels[11], 0);
    }
    #[test]
    fn combine_selection_handles_empty_and_fractional_modes() {
        let mut doc = Document::new(2, 1);
        doc.combine_selection(
            MaskCoverage::Dense(Arc::new(vec![255, 128])),
            SelectionMode::Replace,
        );
        doc.combine_selection(
            MaskCoverage::Dense(Arc::new(vec![0, 128])),
            SelectionMode::Intersect,
        );
        assert_eq!(doc.selection_value(0), 0);
        assert_eq!(doc.selection_value(1), 64);
        doc.combine_selection(
            MaskCoverage::Uniform(255),
            SelectionMode::Subtract,
        );
        assert!(doc.selection.is_none());
    }
}

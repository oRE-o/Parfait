//! Device response calibration, applied before each brush's own pressure curve.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PressureCalibration {
    pub points: Vec<[f32; 2]>,
}
impl Default for PressureCalibration {
    fn default() -> Self {
        Self {
            points: vec![[0., 0.], [1., 1.]],
        }
    }
}
impl PressureCalibration {
    pub fn validate(&self) -> bool {
        (2..=16).contains(&self.points.len())
            && self.points.first() == Some(&[0., 0.])
            && self.points.last() == Some(&[1., 1.])
            && self
                .points
                .iter()
                .flatten()
                .all(|v| v.is_finite() && (0. ..=1.).contains(v))
            && self
                .points
                .windows(2)
                .all(|w| w[1][0] - w[0][0] >= 0.001 && w[1][1] >= w[0][1])
    }
    pub fn map(&self, raw: f32) -> f32 {
        let raw = if raw.is_finite() {
            raw.clamp(0., 1.)
        } else {
            1.
        };
        crate::sample_response(&self.points, raw)
    }
    /// Map the observed 95th percentile to full output, rejecting sparse/flat input.
    pub fn suggest(samples: &[f32]) -> Option<Self> {
        let mut values: Vec<_> = samples
            .iter()
            .copied()
            .filter(|v| v.is_finite() && *v > 0. && *v <= 1.)
            .collect();
        if values.len() < 20 {
            return None;
        }
        values.sort_by(f32::total_cmp);
        let low = values[values.len() / 20];
        let high = values[(values.len() - 1) * 95 / 100];
        if high - low < 0.08 {
            return None;
        }
        let mut result = Self::default();
        if high < 0.999 {
            result.points.insert(1, [high.max(0.01), 1.]);
        }
        Some(result)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn calibration_validates_and_maps_monotonically() {
        let c = PressureCalibration {
            points: vec![[0., 0.], [0.4, 0.8], [0.7, 1.], [1., 1.]],
        };
        assert!(c.validate());
        assert!(c.map(0.2) > 0.4 && c.map(0.2) < 0.8);
        assert_eq!(c.map(0.9), 1.);
        assert!((0..100).all(|i| c.map(i as f32 / 100.) <= c.map((i + 1) as f32 / 100.)));
        let mut invalid = c.clone();
        invalid.points[1][0] = 0.;
        assert!(!invalid.validate());
        invalid = c.clone();
        invalid.points[2][1] = 0.1;
        assert!(!invalid.validate());
        invalid = c;
        invalid.points[1][1] = f32::NAN;
        assert!(!invalid.validate());
    }
    #[test]
    fn suggestion_requires_real_variation_and_rejects_outliers() {
        assert!(PressureCalibration::suggest(&[1.; 100]).is_none());
        assert!(PressureCalibration::suggest(&[0.1, 0.8]).is_none());
        let mut samples: Vec<_> = (0..100).map(|i| 0.1 + i as f32 * 0.005).collect();
        samples.push(1.);
        let c = PressureCalibration::suggest(&samples).unwrap();
        assert!(c.validate());
        assert_eq!(c.map(0.6), 1.);
    }
}

//! Shape-preserving cubic Hermite response through normalized control points.
//! Validated callers provide finite points with strictly increasing x. No heap allocation.
use serde::{Deserialize, Deserializer, Serialize};

pub const MAX_CURVE_POINTS: usize = 16;

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ResponseCurve {
    pub points: Vec<[f32; 2]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub controls: Option<Vec<[[f32; 2]; 2]>>,
}
impl Default for ResponseCurve {
    fn default() -> Self {
        Self {
            points: vec![[0., 0.], [1., 1.]],
            controls: None,
        }
    }
}
impl From<[f32; 5]> for ResponseCurve {
    fn from(samples: [f32; 5]) -> Self {
        Self {
            points: samples
                .into_iter()
                .enumerate()
                .map(|(index, output)| [index as f32 / 4., output])
                .collect(),
            controls: None,
        }
    }
}
impl ResponseCurve {
    pub fn validate(&self) -> bool {
        (2..=MAX_CURVE_POINTS).contains(&self.points.len())
            && self.points.first().is_some_and(|point| point[0] == 0.)
            && self.points.last().is_some_and(|point| point[0] == 1.)
            && self
                .points
                .iter()
                .flatten()
                .all(|value| value.is_finite() && (0. ..=1.).contains(value))
            && self
                .points
                .windows(2)
                .all(|pair| pair[1][0] - pair[0][0] >= 0.001)
            && self.controls.as_ref().is_none_or(|controls| {
                controls.len() + 1 == self.points.len()
                    && controls.iter().enumerate().all(|(index, handles)| {
                        handles
                            .iter()
                            .flatten()
                            .all(|value| value.is_finite() && (0. ..=1.).contains(value))
                            && self.points[index][0] <= handles[0][0]
                            && handles[0][0] <= handles[1][0]
                            && handles[1][0] <= self.points[index + 1][0]
                    })
            })
    }
    pub fn sample(&self, input: f32) -> f32 {
        match &self.controls {
            Some(controls) => sample_bezier_response(&self.points, controls, input),
            None => sample_response(&self.points, input),
        }
    }
}
impl<'de> Deserialize<'de> for ResponseCurve {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Encoded {
            Points {
                points: Vec<[f32; 2]>,
                #[serde(default)]
                controls: Option<Vec<[[f32; 2]; 2]>>,
            },
            Legacy([f32; 5]),
        }
        let curve = match Encoded::deserialize(deserializer)? {
            Encoded::Points { points, controls } => Self { points, controls },
            Encoded::Legacy(samples) => samples.into(),
        };
        if curve.validate() {
            Ok(curve)
        } else {
            Err(serde::de::Error::custom("Invalid response curve"))
        }
    }
}

fn response_tangent(points: &[[f32; 2]], index: usize) -> f32 {
    let slope = |segment: usize| {
        (points[segment + 1][1] - points[segment][1])
            / (points[segment + 1][0] - points[segment][0])
    };
    if index == 0 {
        return slope(0);
    }
    if index + 1 == points.len() {
        return slope(index - 1);
    }
    let left = slope(index - 1);
    let right = slope(index);
    if left * right <= 0. {
        return 0.;
    }
    let before = points[index][0] - points[index - 1][0];
    let after = points[index + 1][0] - points[index][0];
    let first = 2. * after + before;
    let second = after + 2. * before;
    (first + second) / (first / left + second / right)
}

pub fn automatic_curve_controls(points: &[[f32; 2]]) -> Vec<[[f32; 2]; 2]> {
    points
        .windows(2)
        .enumerate()
        .map(|(index, pair)| {
            let span = (pair[1][0] - pair[0][0]) / 3.;
            [
                [
                    pair[0][0] + span,
                    pair[0][1] + span * response_tangent(points, index),
                ],
                [
                    pair[1][0] - span,
                    pair[1][1] - span * response_tangent(points, index + 1),
                ],
            ]
        })
        .collect()
}

pub fn reconcile_curve_controls(
    points: &[[f32; 2]],
    previous: &[[f32; 2]],
    controls: &mut Vec<[[f32; 2]; 2]>,
) {
    let automatic = automatic_curve_controls(points);
    let same_count = points.len() == previous.len();
    *controls = points
        .windows(2)
        .enumerate()
        .map(|(index, pair)| {
            let original = if same_count {
                Some(index)
            } else {
                previous.windows(2).position(|old| old == pair)
            };
            let Some(original) = original.filter(|original| *original < controls.len()) else {
                return automatic[index];
            };
            let mut handles = controls[original];
            for (side, handle) in handles.iter_mut().enumerate() {
                for axis in 0..2 {
                    handle[axis] += pair[side][axis] - previous[original + side][axis];
                }
                handle[0] = handle[0].clamp(pair[0][0], pair[1][0]);
                handle[1] = handle[1].clamp(0., 1.);
            }
            if handles[0][0] > handles[1][0] {
                let middle = (handles[0][0] + handles[1][0]) * 0.5;
                handles[0][0] = middle;
                handles[1][0] = middle;
            }
            handles
        })
        .collect();
}

pub fn sample_bezier_response(points: &[[f32; 2]], controls: &[[[f32; 2]; 2]], input: f32) -> f32 {
    let input = if input.is_finite() { input } else { 1. };
    if input <= points[0][0] {
        return points[0][1];
    }
    if input >= points[points.len() - 1][0] {
        return points[points.len() - 1][1];
    }
    let index = points.partition_point(|point| point[0] <= input) - 1;
    let evaluate = |amount: f32, axis: usize| {
        let inverse = 1. - amount;
        inverse.powi(3) * points[index][axis]
            + 3. * inverse.powi(2) * amount * controls[index][0][axis]
            + 3. * inverse * amount.powi(2) * controls[index][1][axis]
            + amount.powi(3) * points[index + 1][axis]
    };
    let (mut low, mut high) = (0., 1.);
    for _ in 0..24 {
        let middle = (low + high) * 0.5;
        if evaluate(middle, 0) < input {
            low = middle;
        } else {
            high = middle;
        }
    }
    evaluate((low + high) * 0.5, 1).clamp(0., 1.)
}

pub fn sample_response(points: &[[f32; 2]], x: f32) -> f32 {
    if points.is_empty() {
        return x.clamp(0., 1.);
    }
    let x = if x.is_finite() { x } else { 1. };
    if x <= points[0][0] {
        return points[0][1];
    }
    if x >= points[points.len() - 1][0] {
        return points[points.len() - 1][1];
    }
    let i = points.partition_point(|p| p[0] <= x) - 1;
    let h = points[i + 1][0] - points[i][0];
    let t = (x - points[i][0]) / h;
    let a = points[i][1];
    let b = points[i + 1][1];
    let result = (2. * t * t * t - 3. * t * t + 1.) * a
        + (t * t * t - 2. * t * t + t) * h * response_tangent(points, i)
        + (-2. * t * t * t + 3. * t * t) * b
        + (t * t * t - t * t) * h * response_tangent(points, i + 1);
    result.clamp(a.min(b), a.max(b))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bezier_handles_preserve_legacy_shape_and_change_bending_without_moving_anchors() {
        for points in [
            vec![[0., 0.], [1., 1.]],
            vec![[0., 0.], [0.17, 0.9], [0.62, 0.1], [1., 1.]],
            vec![[0., 0.], [0.2, 1.], [0.201, 0.5], [1., 0.]],
        ] {
            let curve = ResponseCurve {
                controls: Some(automatic_curve_controls(&points)),
                points,
            };
            assert!(curve.validate());
            for step in 0..=1000 {
                let input = step as f32 / 1000.;
                assert!(
                    (curve.sample(input) - sample_response(&curve.points, input)).abs() < 0.00002
                );
            }
        }
        let mut curve = ResponseCurve::default();
        let anchors = curve.points.clone();
        curve.controls = Some(vec![[[0.12, 0.9], [0.7, 0.95]]]);
        assert!(curve.sample(0.3) > 0.6);
        assert_eq!(curve.points, anchors);
        for controls in [
            vec![],
            vec![[[0.8, 0.2], [0.2, 0.8]]],
            vec![[[0.2, f32::NAN], [0.7, 0.8]]],
            vec![[[-0.1, 0.2], [0.7, 0.8]]],
        ] {
            curve.controls = Some(controls);
            assert!(!curve.validate());
        }
    }

    #[test]
    fn anchor_edits_keep_unaffected_handles_and_translate_adjacent_handles() {
        let original = vec![[0., 0.], [0.3, 0.4], [0.7, 0.6], [1., 1.]];
        let mut controls = automatic_curve_controls(&original);
        controls[2][0] = [0.8, 0.95];
        let unaffected = controls[2];
        let mut added = original.clone();
        added.insert(1, [0.15, 0.3]);
        reconcile_curve_controls(&added, &original, &mut controls);
        assert_eq!(controls[3], unaffected);
        reconcile_curve_controls(&original, &added, &mut controls);
        assert_eq!(controls[2], unaffected);
        let before = controls[1][0];
        let mut moved = original.clone();
        moved[1] = [0.35, 0.45];
        reconcile_curve_controls(&moved, &original, &mut controls);
        assert!((controls[1][0][0] - before[0] - 0.05).abs() < 0.00001);
        assert_eq!(controls[2], unaffected);
        assert!(
            ResponseCurve {
                points: moved,
                controls: Some(controls)
            }
            .validate()
        );
    }
    #[test]
    fn arbitrary_inputs_and_falling_outputs_are_valid_but_bad_coordinates_are_not() {
        let curve = ResponseCurve {
            points: vec![[0., 0.2], [0.17, 0.9], [0.62, 0.1], [1., 0.7]],
            controls: None,
        };
        assert!(curve.validate());
        assert_eq!(curve.sample(0.17), 0.9);
        assert_eq!(curve.sample(0.62), 0.1);
        for points in [
            vec![],
            vec![[0., 0.]],
            vec![[0.1, 0.], [1., 1.]],
            vec![[0., 0.], [0.0005, 1.], [1., 1.]],
            vec![[0., 0.], [0., 1.]],
            vec![[0., f32::NAN], [1., 1.]],
            vec![[0., 0.], [1., 1.1]],
        ] {
            assert!(
                !ResponseCurve {
                    points,
                    controls: None
                }
                .validate()
            );
        }
        let points: Vec<_> = (0..=16).map(|index| [index as f32 / 16., 0.5]).collect();
        assert!(
            !ResponseCurve {
                points,
                controls: None
            }
            .validate()
        );
        let legacy: ResponseCurve = [0., 0.1, 0.4, 0.9, 1.].into();
        assert!(legacy.validate());
        assert_eq!(legacy.points[2], [0.5, 0.4]);
    }
    #[test]
    fn passes_nodes_is_smooth_and_preserves_shape() {
        for points in [
            vec![[0., 0.], [0.2, 0.05], [0.6, 0.4], [0.85, 1.], [1., 1.]],
            vec![[0., 1.], [0.2, 0.95], [0.6, 0.4], [1., 0.]],
            vec![[0., 0.], [0.2, 1.], [0.201, 0.5], [0.9, 0.5], [1., 0.]],
        ] {
            for p in &points {
                assert!((sample_response(&points, p[0]) - p[1]).abs() < 1e-6);
            }
            for pair in points.windows(2) {
                let mut last = pair[0][1];
                for k in 0..=100 {
                    let x = pair[0][0] + (pair[1][0] - pair[0][0]) * k as f32 / 100.;
                    let y = sample_response(&points, x);
                    assert!(
                        y >= pair[0][1].min(pair[1][1]) - 1e-6
                            && y <= pair[0][1].max(pair[1][1]) + 1e-6
                    );
                    assert!((y - last) * (pair[1][1] - pair[0][1]) >= -1e-6);
                    last = y;
                }
            }
        }
        let points = [[0., 0.], [0.3, 0.1], [0.7, 0.8], [1., 1.]];
        assert!((sample_response(&points, 0.15) - 0.05).abs() > 0.005);
        let e = 0.0001;
        let left = (sample_response(&points, 0.3) - sample_response(&points, 0.3 - e)) / e;
        let right = (sample_response(&points, 0.3 + e) - sample_response(&points, 0.3)) / e;
        assert!((left - right).abs() < 0.01);
        for i in 0..=100 {
            let x = i as f32 / 100.;
            assert!((sample_response(&[[0., 0.], [1., 1.]], x) - x).abs() < 1e-6);
        }
    }
    #[test]
    fn brush_and_calibration_share_the_same_interpolation() {
        let brush = crate::Brush {
            pressure_curve: [0., 0.1, 0.4, 0.9, 1.].into(),
            ..Default::default()
        };
        let points = brush.pressure_curve.points.clone();
        let calibration = crate::PressureCalibration { points };
        for i in 0..=1000 {
            let x = i as f32 / 1000.;
            assert_eq!(brush.mapped_pressure(x), calibration.map(x));
        }
    }
}

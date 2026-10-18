//! Shape-preserving cubic Hermite response through normalized control points.
//! Validated callers provide finite points with strictly increasing x. No heap allocation.
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
    let slope = |j: usize| (points[j + 1][1] - points[j][1]) / (points[j + 1][0] - points[j][0]);
    let tangent = |j: usize| {
        if j == 0 {
            return slope(0);
        }
        if j + 1 == points.len() {
            return slope(j - 1);
        }
        let left = slope(j - 1);
        let right = slope(j);
        if left * right <= 0. {
            return 0.;
        }
        let h0 = points[j][0] - points[j - 1][0];
        let h1 = points[j + 1][0] - points[j][0];
        let w0 = 2. * h1 + h0;
        let w1 = h1 + 2. * h0;
        (w0 + w1) / (w0 / left + w1 / right)
    };
    let h = points[i + 1][0] - points[i][0];
    let t = (x - points[i][0]) / h;
    let a = points[i][1];
    let b = points[i + 1][1];
    let result = (2. * t * t * t - 3. * t * t + 1.) * a
        + (t * t * t - 2. * t * t + t) * h * tangent(i)
        + (-2. * t * t * t + 3. * t * t) * b
        + (t * t * t - t * t) * h * tangent(i + 1);
    result.clamp(a.min(b), a.max(b))
}
#[cfg(test)]
mod tests {
    use super::*;
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
            pressure_curve: [0., 0.1, 0.4, 0.9, 1.],
            ..Default::default()
        };
        let points: Vec<_> = brush
            .pressure_curve
            .iter()
            .enumerate()
            .map(|(i, y)| [i as f32 / 4., *y])
            .collect();
        let calibration = crate::PressureCalibration { points };
        for i in 0..=1000 {
            let x = i as f32 / 1000.;
            assert_eq!(brush.mapped_pressure(x), calibration.map(x));
        }
    }
}

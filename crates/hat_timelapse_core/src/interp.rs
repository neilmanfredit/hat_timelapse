//! Interpolation between keyframes.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Method {
    Linear,
    /// Monotone cubic (Fritsch–Carlson): smooth, but never overshoots the keyframes.
    #[default]
    Smooth,
}

/// Interpolate `points` (x ascending, unique) at every integer x in `0..n`.
/// Values before the first and after the last point are held.
pub fn interpolate(points: &[(f64, f64)], n: usize, method: Method) -> Vec<f64> {
    match points.len() {
        0 => return vec![0.0; n],
        1 => return vec![points[0].1; n],
        _ => {}
    }
    let tangents = match method {
        Method::Linear => None,
        Method::Smooth => Some(monotone_tangents(points)),
    };
    (0..n)
        .map(|i| {
            let x = i as f64;
            if x <= points[0].0 {
                return points[0].1;
            }
            let last = points.len() - 1;
            if x >= points[last].0 {
                return points[last].1;
            }
            let k = points.partition_point(|p| p.0 <= x) - 1;
            let (x0, y0) = points[k];
            let (x1, y1) = points[k + 1];
            let h = x1 - x0;
            let t = (x - x0) / h;
            match &tangents {
                None => y0 + (y1 - y0) * t,
                Some(m) => {
                    let t2 = t * t;
                    let t3 = t2 * t;
                    let h00 = 2.0 * t3 - 3.0 * t2 + 1.0;
                    let h10 = t3 - 2.0 * t2 + t;
                    let h01 = -2.0 * t3 + 3.0 * t2;
                    let h11 = t3 - t2;
                    h00 * y0 + h10 * h * m[k] + h01 * y1 + h11 * h * m[k + 1]
                }
            }
        })
        .collect()
}

fn monotone_tangents(p: &[(f64, f64)]) -> Vec<f64> {
    let n = p.len();
    let d: Vec<f64> = (0..n - 1)
        .map(|k| (p[k + 1].1 - p[k].1) / (p[k + 1].0 - p[k].0))
        .collect();
    let mut m = vec![0.0; n];
    m[0] = d[0];
    m[n - 1] = d[n - 2];
    for k in 1..n - 1 {
        m[k] = if d[k - 1] * d[k] <= 0.0 {
            0.0
        } else {
            (d[k - 1] + d[k]) / 2.0
        };
    }
    for k in 0..n - 1 {
        if d[k] == 0.0 {
            m[k] = 0.0;
            m[k + 1] = 0.0;
            continue;
        }
        let a = m[k] / d[k];
        let b = m[k + 1] / d[k];
        let s = a * a + b * b;
        if s > 9.0 {
            let t = 3.0 / s.sqrt();
            m[k] = t * a * d[k];
            m[k + 1] = t * b * d[k];
        }
    }
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_and_hold() {
        let v = interpolate(&[(2.0, 0.0), (6.0, 4.0)], 9, Method::Linear);
        assert_eq!(v, vec![0.0, 0.0, 0.0, 1.0, 2.0, 3.0, 4.0, 4.0, 4.0]);
    }

    #[test]
    fn smooth_hits_keys_without_overshoot() {
        let pts = [(0.0, 0.0), (10.0, 1.0), (20.0, 1.0), (30.0, 0.0)];
        let v = interpolate(&pts, 31, Method::Smooth);
        for (x, y) in pts {
            assert!((v[x as usize] - y).abs() < 1e-9);
        }
        assert!(v.iter().all(|&y| (-1e-9..=1.0 + 1e-9).contains(&y)));
    }
}

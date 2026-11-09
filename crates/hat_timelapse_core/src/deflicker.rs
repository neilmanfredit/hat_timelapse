//! Visual deflicker.
//!
//! Brightness is measured on each frame, the Hat Timelapse and keyframe exposure
//! already applied is added back in, and the result is compared with a smoothed
//! version of itself. The difference becomes the frame's deflicker correction.
//! A Gaussian over a window of frames keeps real brightness changes (sunset,
//! keyframed ramps) while removing frame-to-frame flicker.
//!
//! Two sources are supported:
//! * originals: the source files (or raw previews). Effective brightness is
//!   predicted by adding the exposure offsets, so one pass is usually enough.
//! * previews: images rendered with the current settings (LightCraft or a
//!   Lightroom export). The measurement already includes the offsets, so the
//!   correction is refined over several passes.

use crate::params::EXPOSURE;
use crate::project::Project;

/// Gaussian smoothing with reflected edges. `window` is the full width in frames.
pub fn smooth(values: &[f64], window: usize) -> Vec<f64> {
    let n = values.len();
    if n < 3 || window < 2 {
        return values.to_vec();
    }
    let sigma = window as f64 / 4.0;
    let radius = (window / 2).max(1) as isize;
    let kernel: Vec<f64> = (-radius..=radius)
        .map(|k| (-(k * k) as f64 / (2.0 * sigma * sigma)).exp())
        .collect();
    let reflect = |i: isize| -> usize {
        let n = n as isize;
        let mut i = i;
        while i < 0 || i >= n {
            if i < 0 {
                i = -i;
            }
            if i >= n {
                i = 2 * (n - 1) - i;
            }
        }
        i as usize
    };
    (0..n as isize)
        .map(|i| {
            let mut s = 0.0;
            let mut w = 0.0;
            for (j, k) in (-radius..=radius).zip(&kernel) {
                s += values[reflect(i + j)] * k;
                w += k;
            }
            s / w
        })
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Originals,
    Previews,
}

/// Update `deflicker_ev` on every included frame from `measured` (log2 luminance,
/// one value per frame; `None` for frames that could not be measured).
/// Returns the largest absolute correction applied in this pass.
pub fn update(
    project: &mut Project,
    measured: &[Option<f64>],
    source: Source,
    window: usize,
) -> f64 {
    let idx: Vec<usize> = project
        .included_indices()
        .into_iter()
        .filter(|&i| measured[i].is_some())
        .collect();
    if idx.len() < 3 {
        return 0.0;
    }
    let effective: Vec<f64> = idx
        .iter()
        .map(|&i| {
            let m = measured[i].unwrap();
            match source {
                Source::Previews => m,
                Source::Originals => {
                    let f = &project.frames[i];
                    m + f.values.get(EXPOSURE).copied().unwrap_or(0.0) + f.offset_ev()
                }
            }
        })
        .collect();
    let target = smooth(&effective, window);
    let mut max = 0.0f64;
    for (k, &i) in idx.iter().enumerate() {
        let d = target[k] - effective[k];
        let f = &mut project.frames[i];
        f.deflicker_ev += d;
        max = max.max(d.abs());
    }
    max
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::Frame;
    use std::path::Path;

    #[test]
    #[allow(clippy::needless_range_loop)]
    fn removes_flicker_keeps_ramp() {
        let n = 200;
        let frames = (0..n)
            .map(|i| Frame::new(format!("{i}.jpg"), Default::default()))
            .collect();
        let mut p = Project::new(Path::new("."), frames);
        // A slow 3-stop sunset ramp plus ±0.2 EV alternating flicker.
        let ramp = |i: usize| -3.0 * i as f64 / (n - 1) as f64;
        let measured: Vec<Option<f64>> = (0..n)
            .map(|i| Some(ramp(i) + if i % 2 == 0 { 0.2 } else { -0.2 }))
            .collect();
        update(&mut p, &measured, Source::Originals, 15);
        for i in 20..n - 20 {
            let out = measured[i].unwrap() + p.frames[i].deflicker_ev;
            assert!(
                (out - ramp(i)).abs() < 0.02,
                "frame {i}: {out} vs {}",
                ramp(i)
            );
        }
    }
}

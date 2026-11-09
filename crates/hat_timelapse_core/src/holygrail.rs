//! Holy grail: compensating for camera setting changes through day-to-night shots.
//!
//! When the photographer (or a ramping controller) changes shutter, aperture or
//! ISO, the image jumps in brightness. Each frame gets an exposure offset that
//! brings it back to the reference frame's camera settings, so the brightness
//! follows the scene, not the camera dial. The overall brightness curve is then
//! shaped with Exposure keyframes, and visual deflicker cleans up the remainder.

use crate::project::Project;

#[derive(Debug, Clone, PartialEq)]
pub struct Step {
    pub frame: usize,
    /// Change in camera EV from the previous frame (positive = darker settings).
    pub delta_ev: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Report {
    pub steps: Vec<Step>,
    pub missing_exif: usize,
    pub min_offset: f64,
    pub max_offset: f64,
}

/// Camera EV per frame, filling gaps from the nearest frame that has metadata.
pub fn ev_series(project: &Project) -> (Vec<f64>, usize) {
    let raw: Vec<Option<f64>> = project.frames.iter().map(|f| f.exif.ev()).collect();
    let missing = raw.iter().filter(|v| v.is_none()).count();
    let mut out = vec![0.0; raw.len()];
    let mut last = raw.iter().flatten().next().copied().unwrap_or(0.0);
    for (i, v) in raw.iter().enumerate() {
        if let Some(v) = v {
            last = *v;
        }
        out[i] = last;
    }
    (out, missing)
}

/// Compute `holy_grail_ev` for every frame relative to `reference`.
/// `threshold` ignores tiny EV differences from rounding in the metadata.
pub fn apply(project: &mut Project, reference: usize, threshold: f64) -> Report {
    let (ev, missing) = ev_series(project);
    let reference = reference.min(ev.len().saturating_sub(1));
    let base = ev.get(reference).copied().unwrap_or(0.0);
    let mut steps = Vec::new();
    for i in 1..ev.len() {
        let d = ev[i] - ev[i - 1];
        if d.abs() >= threshold {
            steps.push(Step {
                frame: i,
                delta_ev: d,
            });
        }
    }
    let (mut lo, mut hi) = (0.0f64, 0.0f64);
    for (f, e) in project.frames.iter_mut().zip(&ev) {
        let off = e - base;
        f.holy_grail_ev = if off.abs() < threshold { 0.0 } else { off };
        lo = lo.min(f.holy_grail_ev);
        hi = hi.max(f.holy_grail_ev);
    }
    Report {
        steps,
        missing_exif: missing,
        min_offset: lo,
        max_offset: hi,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exif::ExposureInfo;
    use crate::project::Frame;
    use std::path::Path;

    #[test]
    fn compensates_shutter_steps() {
        let shutters = [1.0 / 100.0, 1.0 / 100.0, 1.0 / 50.0, 1.0 / 50.0, 1.0 / 25.0];
        let frames = shutters
            .iter()
            .enumerate()
            .map(|(i, &t)| {
                Frame::new(
                    format!("{i}.jpg"),
                    ExposureInfo {
                        shutter_s: Some(t),
                        aperture: Some(8.0),
                        iso: Some(100.0),
                        captured: None,
                    },
                )
            })
            .collect();
        let mut p = Project::new(Path::new("."), frames);
        let r = apply(&mut p, 0, 0.05);
        assert_eq!(r.steps.len(), 2);
        let offs: Vec<f64> = p
            .frames
            .iter()
            .map(|f| (f.holy_grail_ev * 100.0).round() / 100.0)
            .collect();
        // Longer shutter = brighter capture = negative compensation.
        assert_eq!(offs, vec![0.0, 0.0, -1.0, -1.0, -2.0]);
    }
}

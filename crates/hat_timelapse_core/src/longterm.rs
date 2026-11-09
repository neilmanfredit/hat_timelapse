//! Filters for long-term and construction timelapses.
//!
//! Months of captures from a fixed camera contain nights, weekends, rain on the
//! lens and the odd person walking in front of it. These filters mark frames as
//! excluded; nothing is deleted. They can be combined and re-run.

use crate::exif::days_from_civil;
use crate::project::Project;

/// Seconds since midnight and weekday (0 = Monday) of a frame's capture time.
fn clock(project: &Project, i: usize) -> Option<(f64, u32)> {
    let s = project.frames[i].exif.captured.as_deref()?;
    let (date, _) = s.trim().split_once(' ')?;
    let mut d = date.split(':').map(|x| x.parse::<i64>().ok());
    let (y, m, dd) = (d.next()??, d.next()??, d.next()??);
    let days = days_from_civil(y, m, dd);
    let secs = crate::exif::parse_exif_time(s)? - days as f64 * 86400.0;
    // 1970-01-01 was a Thursday (index 3 when Monday = 0).
    let weekday = (days + 3).rem_euclid(7) as u32;
    Some((secs, weekday))
}

/// Keep only frames captured between `from` and `to` (hours, 0..24; may wrap midnight).
pub fn time_of_day(project: &mut Project, from: f64, to: f64) -> usize {
    let mut n = 0;
    for i in 0..project.frames.len() {
        if let Some((secs, _)) = clock(project, i) {
            let h = secs / 3600.0;
            let inside = if from <= to {
                h >= from && h < to
            } else {
                h >= from || h < to
            };
            if !inside && !project.frames[i].excluded {
                project.frames[i].excluded = true;
                n += 1;
            }
        }
    }
    n
}

/// Exclude frames captured on the given weekdays (0 = Monday … 6 = Sunday).
pub fn weekdays(project: &mut Project, drop: &[u32]) -> usize {
    let mut n = 0;
    for i in 0..project.frames.len() {
        if let Some((_, wd)) = clock(project, i) {
            if drop.contains(&wd) && !project.frames[i].excluded {
                project.frames[i].excluded = true;
                n += 1;
            }
        }
    }
    n
}

/// Keep one frame per `period_s` bucket of capture time: the frame closest to the
/// bucket's median brightness (steady light, no passing shadows).
/// Frames without luminance fall back to the first in each bucket.
pub fn one_per_period(project: &mut Project, period_s: f64) -> usize {
    use std::collections::BTreeMap;
    let mut buckets: BTreeMap<i64, Vec<usize>> = BTreeMap::new();
    for i in project.included_indices() {
        if let Some(t) = project.frames[i].exif.captured_seconds() {
            buckets
                .entry((t / period_s).floor() as i64)
                .or_default()
                .push(i);
        }
    }
    let mut n = 0;
    for frames in buckets.values() {
        let lums: Vec<f64> = frames
            .iter()
            .filter_map(|&i| project.frames[i].luminance)
            .collect();
        let keep = if lums.len() == frames.len() && !lums.is_empty() {
            let med = median(&lums);
            *frames
                .iter()
                .min_by(|&&a, &&b| {
                    let da = (project.frames[a].luminance.unwrap() - med).abs();
                    let db = (project.frames[b].luminance.unwrap() - med).abs();
                    da.total_cmp(&db)
                })
                .unwrap()
        } else {
            frames[0]
        };
        for &i in frames {
            if i != keep {
                project.frames[i].excluded = true;
                n += 1;
            }
        }
    }
    n
}

/// Exclude frames whose brightness differs from the running median of their
/// neighbours by more than `max_ev` stops (lens covered, flash, dark rain cloud).
pub fn brightness_outliers(project: &mut Project, window: usize, max_ev: f64) -> usize {
    let idx: Vec<usize> = project
        .included_indices()
        .into_iter()
        .filter(|&i| project.frames[i].luminance.is_some())
        .collect();
    let l: Vec<f64> = idx
        .iter()
        .map(|&i| project.frames[i].luminance.unwrap())
        .collect();
    let half = window.max(3) / 2;
    let mut drop = Vec::new();
    for k in 0..l.len() {
        let lo = k.saturating_sub(half);
        let hi = (k + half + 1).min(l.len());
        if (l[k] - median(&l[lo..hi])).abs() > max_ev {
            drop.push(idx[k]);
        }
    }
    for &i in &drop {
        project.frames[i].excluded = true;
    }
    drop.len()
}

pub fn reset(project: &mut Project) {
    for f in &mut project.frames {
        f.excluded = false;
    }
}

fn median(v: &[f64]) -> f64 {
    let mut s = v.to_vec();
    s.sort_by(f64::total_cmp);
    let m = s.len() / 2;
    if s.len().is_multiple_of(2) {
        (s[m - 1] + s[m]) / 2.0
    } else {
        s[m]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exif::ExposureInfo;
    use crate::project::Frame;
    use std::path::Path;

    fn at(ts: &str) -> Frame {
        Frame::new(
            ts.into(),
            ExposureInfo {
                captured: Some(ts.into()),
                ..Default::default()
            },
        )
    }

    #[test]
    fn working_hours_on_weekdays() {
        // 2026-11-07 is a Saturday, 2026-11-09 a Monday.
        let mut p = Project::new(
            Path::new("."),
            vec![
                at("2026:11:07 12:00:00"),
                at("2026:11:09 03:00:00"),
                at("2026:11:09 12:00:00"),
                at("2026:11:09 23:30:00"),
            ],
        );
        assert_eq!(weekdays(&mut p, &[5, 6]), 1);
        assert_eq!(time_of_day(&mut p, 7.0, 18.0), 2);
        assert_eq!(p.included_indices(), vec![2]);
    }

    #[test]
    fn rejects_outliers() {
        let mut p = Project::new(
            Path::new("."),
            (0..20)
                .map(|i| Frame::new(i.to_string(), Default::default()))
                .collect(),
        );
        for (i, f) in p.frames.iter_mut().enumerate() {
            f.luminance = Some(if i == 7 { -6.0 } else { -2.0 });
        }
        assert_eq!(brightness_outliers(&mut p, 9, 1.0), 1);
        assert!(p.frames[7].excluded);
    }
}

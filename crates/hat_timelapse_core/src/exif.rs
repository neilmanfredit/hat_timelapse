//! Capture metadata: shutter, aperture, ISO and capture time.
//!
//! Uses the pure-Rust EXIF reader first (JPEG, TIFF and TIFF-based raws such as
//! DNG, CR2, NEF, ARW, ORF, PEF, RW2). If that fails and `exiftool` is on the PATH
//! it is used as a fallback, which covers CR3, RAF and others.

use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ExposureInfo {
    pub shutter_s: Option<f64>,
    pub aperture: Option<f64>,
    pub iso: Option<f64>,
    /// `YYYY:MM:DD HH:MM:SS[.sss]` as written by the camera.
    pub captured: Option<String>,
}

impl ExposureInfo {
    /// Exposure value of the camera settings, normalised to ISO 100.
    /// One stop more light reaching the image = EV one lower.
    pub fn ev(&self) -> Option<f64> {
        let (t, n, iso) = (self.shutter_s?, self.aperture?, self.iso.unwrap_or(100.0));
        if t <= 0.0 || n <= 0.0 || iso <= 0.0 {
            return None;
        }
        Some((n * n / t).log2() - (iso / 100.0).log2())
    }

    pub fn is_complete(&self) -> bool {
        self.shutter_s.is_some() && self.aperture.is_some() && self.iso.is_some()
    }

    /// Seconds since 0000-01-01 (proleptic, good enough for ordering and gaps).
    pub fn captured_seconds(&self) -> Option<f64> {
        parse_exif_time(self.captured.as_deref()?)
    }
}

pub fn parse_exif_time(s: &str) -> Option<f64> {
    let s = s.trim();
    let (date, time) = s.split_once(' ')?;
    let mut d = date.split(':').map(|x| x.parse::<i64>().ok());
    let (y, mo, da) = (d.next()??, d.next()??, d.next()??);
    let mut t = time.split(':');
    let h: f64 = t.next()?.parse().ok()?;
    let mi: f64 = t.next()?.parse().ok()?;
    let sec: f64 = t.next()?.trim_end_matches('Z').parse().ok()?;
    Some(days_from_civil(y, mo, da) as f64 * 86400.0 + h * 3600.0 + mi * 60.0 + sec)
}

/// Days since 1970-01-01 (Howard Hinnant's algorithm).
pub fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

pub fn read(path: &Path) -> ExposureInfo {
    let info = read_native(path).unwrap_or_default();
    if info.is_complete() && info.captured.is_some() {
        return info;
    }
    match read_exiftool(path) {
        Some(e) => ExposureInfo {
            shutter_s: info.shutter_s.or(e.shutter_s),
            aperture: info.aperture.or(e.aperture),
            iso: info.iso.or(e.iso),
            captured: info.captured.or(e.captured),
        },
        None => info,
    }
}

fn read_native(path: &Path) -> Option<ExposureInfo> {
    use exif::{In, Tag, Value};
    let file = std::fs::File::open(path).ok()?;
    let mut r = std::io::BufReader::new(file);
    let ex = exif::Reader::new().read_from_container(&mut r).ok()?;
    let num = |tag: Tag| -> Option<f64> {
        let f = ex.get_field(tag, In::PRIMARY)?;
        match &f.value {
            Value::Rational(v) => v.first().map(|r| r.to_f64()),
            Value::SRational(v) => v.first().map(|r| r.to_f64()),
            Value::Short(v) => v.first().map(|&x| x as f64),
            Value::Long(v) => v.first().map(|&x| x as f64),
            _ => None,
        }
    };
    let text = |tag: Tag| -> Option<String> {
        let f = ex.get_field(tag, In::PRIMARY)?;
        match &f.value {
            Value::Ascii(v) => v
                .first()
                .map(|b| String::from_utf8_lossy(b).trim().to_string()),
            _ => None,
        }
    };
    let mut captured = text(Tag::DateTimeOriginal).or_else(|| text(Tag::DateTime));
    if let (Some(c), Some(sub)) = (&mut captured, text(Tag::SubSecTimeOriginal)) {
        let sub = sub.trim();
        if !sub.is_empty() && sub.chars().all(|c| c.is_ascii_digit()) {
            c.push('.');
            c.push_str(sub);
        }
    }
    Some(ExposureInfo {
        shutter_s: num(Tag::ExposureTime),
        aperture: num(Tag::FNumber),
        iso: num(Tag::PhotographicSensitivity),
        captured,
    })
}

fn read_exiftool(path: &Path) -> Option<ExposureInfo> {
    let out = Command::new("exiftool")
        .args([
            "-j",
            "-n",
            "-ExposureTime",
            "-FNumber",
            "-ISO",
            "-SubSecDateTimeOriginal",
            "-DateTimeOriginal",
        ])
        .arg(path)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).ok()?;
    let o = v.get(0)?;
    let f = |k: &str| o.get(k).and_then(|x| x.as_f64());
    let captured = o
        .get("SubSecDateTimeOriginal")
        .or_else(|| o.get("DateTimeOriginal"))
        .and_then(|x| x.as_str())
        .map(|s| s.split(['+', '-']).next().unwrap_or(s).to_string());
    Some(ExposureInfo {
        shutter_s: f("ExposureTime"),
        aperture: f("FNumber"),
        iso: f("ISO"),
        captured,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ev_maths() {
        let e = ExposureInfo {
            shutter_s: Some(1.0),
            aperture: Some(1.0),
            iso: Some(100.0),
            captured: None,
        };
        assert_eq!(e.ev(), Some(0.0));
        let e2 = ExposureInfo {
            shutter_s: Some(2.0),
            ..e.clone()
        };
        assert!((e2.ev().unwrap() + 1.0).abs() < 1e-12);
        let e3 = ExposureInfo {
            iso: Some(200.0),
            ..e
        };
        assert!((e3.ev().unwrap() + 1.0).abs() < 1e-12);
    }

    #[test]
    fn time_parsing() {
        let a = parse_exif_time("2026:06:21 21:30:00").unwrap();
        let b = parse_exif_time("2026:06:22 00:30:00.5").unwrap();
        assert!((b - a - 3.0 * 3600.0 - 0.5).abs() < 1e-6);
        assert_eq!(days_from_civil(1970, 1, 1), 0);
    }
}

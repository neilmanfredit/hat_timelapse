//! Measuring frame brightness for deflicker and long-term filtering.
//!
//! Raster files (JPEG, PNG, TIFF) are decoded directly. For raw files the largest
//! embedded JPEG preview is used, which every mainstream camera writes. Brightness
//! is reported as log2 of mean linear luminance, so differences are in stops (EV).

use anyhow::{anyhow, Context, Result};
use image::{DynamicImage, ImageFormat, ImageReader};
use std::io::Cursor;
use std::path::Path;

/// Region of interest, normalised 0..1: x, y, width, height.
pub type Roi = [f64; 4];

const MAX_EDGE: u32 = 512;

pub fn load_for_analysis(path: &Path) -> Result<DynamicImage> {
    let img = if crate::sequence::is_raw(path) {
        let bytes = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
        largest_embedded_jpeg(&bytes)
            .ok_or_else(|| anyhow!("{}: no embedded preview found", path.display()))?
    } else {
        ImageReader::open(path)?
            .with_guessed_format()?
            .decode()
            .with_context(|| format!("decoding {}", path.display()))?
    };
    Ok(if img.width().max(img.height()) > MAX_EDGE {
        img.thumbnail(MAX_EDGE, MAX_EDGE)
    } else {
        img
    })
}

/// Find the biggest JPEG inside a raw file. JPEG decoders stop at the end-of-image
/// marker, so each SOI candidate can be tried against the rest of the buffer.
pub fn largest_embedded_jpeg(bytes: &[u8]) -> Option<DynamicImage> {
    let mut best: Option<(u64, usize)> = None;
    let mut i = 0;
    while i + 3 < bytes.len() {
        if bytes[i] == 0xFF && bytes[i + 1] == 0xD8 && bytes[i + 2] == 0xFF {
            let r = ImageReader::with_format(Cursor::new(&bytes[i..]), ImageFormat::Jpeg);
            if let Ok((w, h)) = r.into_dimensions() {
                let area = w as u64 * h as u64;
                if best.is_none_or(|(a, _)| area > a) {
                    best = Some((area, i));
                }
            }
        }
        i += 1;
    }
    let (_, off) = best?;
    ImageReader::with_format(Cursor::new(&bytes[off..]), ImageFormat::Jpeg)
        .decode()
        .ok()
}

fn srgb_to_linear(v: f32) -> f32 {
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

/// log2 of mean linear luminance (Rec. 709 weights) inside `roi`.
/// Clipped highlights and pure black are ignored so they do not drag the mean.
pub fn measure(img: &DynamicImage, roi: Option<Roi>) -> Option<f64> {
    let rgb = img.to_rgb32f();
    let (w, h) = rgb.dimensions();
    let [rx, ry, rw, rh] = roi.unwrap_or([0.0, 0.0, 1.0, 1.0]);
    let x0 = ((rx * w as f64) as u32).min(w.saturating_sub(1));
    let y0 = ((ry * h as f64) as u32).min(h.saturating_sub(1));
    let x1 = (((rx + rw) * w as f64).ceil() as u32).clamp(x0 + 1, w);
    let y1 = (((ry + rh) * h as f64).ceil() as u32).clamp(y0 + 1, h);
    let (mut sum, mut n) = (0.0f64, 0u64);
    for y in y0..y1 {
        for x in x0..x1 {
            let p = rgb.get_pixel(x, y).0;
            if p.iter().any(|&c| c >= 0.995) {
                continue;
            }
            let l = 0.2126 * srgb_to_linear(p[0])
                + 0.7152 * srgb_to_linear(p[1])
                + 0.0722 * srgb_to_linear(p[2]);
            if l > 1e-5 {
                sum += l as f64;
                n += 1;
            }
        }
    }
    (n > 0).then(|| (sum / n as f64).log2())
}

pub fn measure_file(path: &Path, roi: Option<Roi>) -> Result<f64> {
    let img = load_for_analysis(path)?;
    measure(&img, roi)
        .ok_or_else(|| anyhow!("{}: image is entirely clipped or black", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgb, RgbImage};

    #[test]
    fn one_stop_brighter_measures_plus_one() {
        // Linear 0.1 and 0.2 encoded as sRGB.
        let enc = |l: f32| -> u8 {
            let v = if l <= 0.0031308 {
                l * 12.92
            } else {
                1.055 * l.powf(1.0 / 2.4) - 0.055
            };
            (v * 255.0).round() as u8
        };
        let a = DynamicImage::ImageRgb8(RgbImage::from_pixel(8, 8, Rgb([enc(0.1); 3])));
        let b = DynamicImage::ImageRgb8(RgbImage::from_pixel(8, 8, Rgb([enc(0.2); 3])));
        let d = measure(&b, None).unwrap() - measure(&a, None).unwrap();
        assert!((d - 1.0).abs() < 0.03, "{d}");
    }

    #[test]
    fn finds_embedded_jpeg() {
        let img = DynamicImage::ImageRgb8(RgbImage::from_pixel(64, 32, Rgb([120, 120, 120])));
        let mut jpeg = Vec::new();
        img.write_to(&mut Cursor::new(&mut jpeg), ImageFormat::Jpeg)
            .unwrap();
        let mut raw = vec![0u8; 1000];
        raw.extend_from_slice(&jpeg);
        raw.extend_from_slice(&[0u8; 500]);
        let found = largest_embedded_jpeg(&raw).unwrap();
        assert_eq!((found.width(), found.height()), (64, 32));
    }
}

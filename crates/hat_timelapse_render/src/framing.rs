//! Framing: output aspect crop, pan/zoom, scaling and colour conversion into a
//! numbered TIFF sequence (`frame_000001.tif`, …). This sequence feeds the video
//! encoder and is also the hand-off to EffectCraft and FilmCraft.

use crate::colour::{self, InputSpace, OutputSpace};
use anyhow::{bail, Context, Result};
use hat_timelapse_core::MotionKey;
use image::imageops::{self, FilterType};
use image::{DynamicImage, ImageBuffer, ImageReader, Rgb};
use rayon::prelude::*;
use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Size {
    pub width: u32,
    pub height: u32,
}

/// Named output sizes. Any `WIDTHxHEIGHT` also works, with no upper limit.
pub const PRESETS: &[(&str, u32, u32)] = &[
    ("hd", 1280, 720),
    ("fhd", 1920, 1080),
    ("1080p", 1920, 1080),
    ("2.7k", 2704, 1520),
    ("uhd", 3840, 2160),
    ("4k", 3840, 2160),
    ("dci4k", 4096, 2160),
    ("5k", 5120, 2880),
    ("6k", 6144, 3456),
    ("8k", 7680, 4320),
    ("dci8k", 8192, 4320),
    ("12k", 12288, 6480),
];

impl Size {
    pub fn parse(s: &str) -> Result<Size> {
        let l = s.to_ascii_lowercase();
        if let Some((_, w, h)) = PRESETS.iter().find(|(n, _, _)| *n == l) {
            return Ok(Size {
                width: *w,
                height: *h,
            });
        }
        if let Some((w, h)) = l.split_once('x') {
            let (w, h): (u32, u32) = (w.trim().parse()?, h.trim().parse()?);
            if w < 16 || h < 16 {
                bail!("output size {w}x{h} is too small");
            }
            // Encoders need even dimensions for 4:2:0 / 4:2:2.
            return Ok(Size {
                width: w & !1,
                height: h & !1,
            });
        }
        bail!("unknown size `{s}` (use a preset such as fhd, 4k, 8k or WIDTHxHEIGHT)")
    }
}

/// Crop rectangle (x, y, w, h) in source pixels for a given output aspect and motion.
pub fn crop_rect(src_w: u32, src_h: u32, out: Size, m: &MotionKey) -> (u32, u32, u32, u32) {
    let (sw, sh) = (src_w as f64, src_h as f64);
    let aspect = out.width as f64 / out.height as f64;
    let (mut cw, mut ch) = if sw / sh > aspect {
        (sh * aspect, sh)
    } else {
        (sw, sw / aspect)
    };
    let z = m.zoom.max(1.0);
    cw /= z;
    ch /= z;
    let x = (m.cx * sw - cw / 2.0).clamp(0.0, sw - cw);
    let y = (m.cy * sh - ch / 2.0).clamp(0.0, sh - ch);
    let cw = (cw.round() as u32).clamp(1, src_w);
    let ch = (ch.round() as u32).clamp(1, src_h);
    (
        (x.round() as u32).min(src_w - cw),
        (y.round() as u32).min(src_h - ch),
        cw,
        ch,
    )
}

#[derive(Debug, Clone)]
pub struct FrameJob {
    pub input: PathBuf,
    pub motion: MotionKey,
}

#[derive(Debug, Clone)]
pub struct Options {
    pub size: Size,
    pub output: OutputSpace,
    /// `None` = detect: float TIFF → linear, everything else → sRGB.
    pub input: Option<InputSpace>,
    pub prefix: String,
    pub overwrite: bool,
}

#[derive(Debug, Serialize)]
pub struct Manifest {
    pub generator: String,
    pub frames: usize,
    pub pattern: String,
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub colour_space: OutputSpace,
    pub bits_per_channel: u32,
    pub sources: Vec<String>,
}

pub fn output_name(prefix: &str, n: usize) -> String {
    format!("{prefix}_{:06}.tif", n + 1)
}

/// Frame every job into `out_dir`. Returns the number of frames written.
/// `progress(done, total)` is called as frames complete.
pub fn run(
    jobs: &[FrameJob],
    out_dir: &Path,
    opts: &Options,
    fps: f64,
    progress: impl Fn(usize, usize) + Sync,
) -> Result<usize> {
    if jobs.is_empty() {
        bail!("nothing to frame: no rendered images found");
    }
    std::fs::create_dir_all(out_dir)?;
    let done = std::sync::atomic::AtomicUsize::new(0);
    jobs.par_iter()
        .enumerate()
        .try_for_each(|(n, job)| -> Result<()> {
            let dst = out_dir.join(output_name(&opts.prefix, n));
            if opts.overwrite || !dst.exists() {
                frame_one(&job.input, &dst, &job.motion, opts)
                    .with_context(|| format!("framing {}", job.input.display()))?;
            }
            progress(
                done.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1,
                jobs.len(),
            );
            Ok(())
        })?;
    let manifest = Manifest {
        generator: format!("Hat Timelapse {}", env!("CARGO_PKG_VERSION")),
        frames: jobs.len(),
        pattern: format!("{}_%06d.tif", opts.prefix),
        width: opts.size.width,
        height: opts.size.height,
        fps,
        colour_space: opts.output,
        bits_per_channel: if opts.output == OutputSpace::Linear {
            32
        } else {
            16
        },
        sources: jobs
            .iter()
            .map(|j| j.input.file_name().unwrap().to_string_lossy().into_owned())
            .collect(),
    };
    std::fs::write(
        out_dir.join("sequence.json"),
        serde_json::to_string_pretty(&manifest)?,
    )?;
    Ok(jobs.len())
}

fn frame_one(src: &Path, dst: &Path, m: &MotionKey, opts: &Options) -> Result<()> {
    let img = ImageReader::open(src)?.with_guessed_format()?.decode()?;
    let is_float = matches!(
        img,
        DynamicImage::ImageRgb32F(_) | DynamicImage::ImageRgba32F(_)
    );
    let input = opts.input.unwrap_or(if is_float {
        InputSpace::LinearRec709
    } else {
        InputSpace::Srgb
    });
    let (x, y, w, h) = crop_rect(img.width(), img.height(), opts.size, m);
    let (tw, th) = (opts.size.width, opts.size.height);
    let filter = if w > tw {
        FilterType::Lanczos3
    } else {
        FilterType::CatmullRom
    };

    // Resize in the source's own precision, then convert colour at output size.
    let scaled: ImageBuffer<Rgb<f32>, Vec<f32>> = if is_float {
        let buf = img.to_rgb32f();
        let c = imageops::crop_imm(&buf, x, y, w, h).to_image();
        imageops::resize(&c, tw, th, filter)
    } else {
        let buf = img.to_rgb16();
        let c = imageops::crop_imm(&buf, x, y, w, h).to_image();
        DynamicImage::ImageRgb16(imageops::resize(&c, tw, th, filter)).to_rgb32f()
    };
    drop(img);

    let tmp = dst.with_extension("tif.part");
    if opts.output == OutputSpace::Linear {
        let mut out = scaled;
        for p in out.pixels_mut() {
            p.0 = colour::convert(p.0, input, OutputSpace::Linear);
        }
        DynamicImage::ImageRgb32F(out).save_with_format(&tmp, image::ImageFormat::Tiff)?;
    } else {
        let out: ImageBuffer<Rgb<u16>, Vec<u16>> = ImageBuffer::from_fn(tw, th, |px, py| {
            let c = colour::convert(scaled.get_pixel(px, py).0, input, opts.output);
            Rgb(c.map(|v| (v.clamp(0.0, 1.0) * 65535.0).round() as u16))
        });
        DynamicImage::ImageRgb16(out).save_with_format(&tmp, image::ImageFormat::Tiff)?;
    }
    std::fs::rename(&tmp, dst)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(zoom: f64, cx: f64, cy: f64) -> MotionKey {
        MotionKey {
            frame: 0,
            zoom,
            cx,
            cy,
        }
    }

    #[test]
    fn crops_3_2_to_16_9() {
        let fhd = Size::parse("fhd").unwrap();
        assert_eq!(
            crop_rect(6000, 4000, fhd, &m(1.0, 0.5, 0.5)),
            (0, 313, 6000, 3375)
        );
        // Zoom 2x into the top-left corner is clamped inside the frame.
        assert_eq!(
            crop_rect(6000, 4000, fhd, &m(2.0, 0.0, 0.0)),
            (0, 0, 3000, 1688)
        );
    }

    #[test]
    fn sizes() {
        assert_eq!(
            Size::parse("8K").unwrap(),
            Size {
                width: 7680,
                height: 4320
            }
        );
        assert_eq!(
            Size::parse("10241x5761").unwrap(),
            Size {
                width: 10240,
                height: 5760
            }
        );
        assert!(Size::parse("huge").is_err());
    }
}

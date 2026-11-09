//! Encoding the framed TIFF sequence to video with ffmpeg.
//!
//! ffmpeg is used because it is on every platform and carries the reference
//! encoders for each format. Hat Timelapse has already done the colour work, so
//! ffmpeg only converts RGB to YUV with the right matrix and tags the stream.

use crate::colour::OutputSpace;
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Codec {
    /// H.264 8-bit 4:2:0, MP4. Plays everywhere; SDR only.
    H264,
    /// H.265/HEVC 10-bit 4:2:0, MP4. HDR10 and HLG capable.
    Hevc,
    /// ProRes 422 HQ 10-bit, MOV.
    Prores422Hq,
    /// ProRes 4444 12-bit, MOV.
    Prores4444,
    /// ProRes 4444 XQ 12-bit, MOV.
    Prores4444Xq,
    /// DNxHR HQ 8-bit 4:2:2, MOV.
    DnxhrHq,
    /// DNxHR HQX 10-bit 4:2:2, MOV.
    DnxhrHqx,
    /// DNxHR 444 10-bit 4:4:4, MOV.
    Dnxhr444,
    /// AV1 10-bit (SVT-AV1), MP4.
    Av1,
}

impl Codec {
    pub fn extension(self) -> &'static str {
        match self {
            Codec::H264 | Codec::Hevc | Codec::Av1 => "mp4",
            _ => "mov",
        }
    }

    pub fn encoder(self) -> &'static str {
        match self {
            Codec::H264 => "libx264",
            Codec::Hevc => "libx265",
            Codec::Av1 => "libsvtav1",
            Codec::Prores422Hq | Codec::Prores4444 | Codec::Prores4444Xq => "prores_ks",
            Codec::DnxhrHq | Codec::DnxhrHqx | Codec::Dnxhr444 => "dnxhd",
        }
    }

    pub fn supports_hdr(self) -> bool {
        !matches!(self, Codec::H264 | Codec::DnxhrHq)
    }

    fn args(self, quality: u32) -> Vec<String> {
        let q = quality.to_string();
        let v: Vec<&str> = match self {
            Codec::H264 => vec![
                "-c:v",
                "libx264",
                "-preset",
                "slow",
                "-crf",
                &q,
                "-pix_fmt",
                "yuv420p",
                "-profile:v",
                "high",
            ],
            Codec::Hevc => vec![
                "-c:v",
                "libx265",
                "-preset",
                "slow",
                "-crf",
                &q,
                "-pix_fmt",
                "yuv420p10le",
                "-tag:v",
                "hvc1",
            ],
            Codec::Av1 => vec![
                "-c:v",
                "libsvtav1",
                "-preset",
                "6",
                "-crf",
                &q,
                "-pix_fmt",
                "yuv420p10le",
            ],
            Codec::Prores422Hq => vec![
                "-c:v",
                "prores_ks",
                "-profile:v",
                "3",
                "-pix_fmt",
                "yuv422p10le",
                "-vendor",
                "apl0",
            ],
            Codec::Prores4444 => vec![
                "-c:v",
                "prores_ks",
                "-profile:v",
                "4",
                "-pix_fmt",
                "yuv444p12le",
                "-vendor",
                "apl0",
            ],
            Codec::Prores4444Xq => vec![
                "-c:v",
                "prores_ks",
                "-profile:v",
                "5",
                "-pix_fmt",
                "yuv444p12le",
                "-vendor",
                "apl0",
            ],
            Codec::DnxhrHq => vec![
                "-c:v",
                "dnxhd",
                "-profile:v",
                "dnxhr_hq",
                "-pix_fmt",
                "yuv422p",
            ],
            Codec::DnxhrHqx => vec![
                "-c:v",
                "dnxhd",
                "-profile:v",
                "dnxhr_hqx",
                "-pix_fmt",
                "yuv422p10le",
            ],
            Codec::Dnxhr444 => vec![
                "-c:v",
                "dnxhd",
                "-profile:v",
                "dnxhr_444",
                "-pix_fmt",
                "yuv444p10le",
            ],
        };
        v.into_iter().map(String::from).collect()
    }
}

#[derive(Debug, Clone)]
pub struct Options {
    pub codec: Codec,
    pub fps: f64,
    /// CRF for H.264/HEVC/AV1 (lower = better). Ignored by ProRes/DNxHR.
    pub quality: u32,
    pub colour: OutputSpace,
    /// Blend this many consecutive frames for motion blur (1 = off).
    pub motion_blur: u32,
    /// HDR10 MaxCLL / MaxFALL in nits.
    pub max_cll: (u32, u32),
    pub ffmpeg: String,
    /// Output is larger than 4K DCI; use memory-saving encoder settings.
    pub large_frame: bool,
}

pub fn build_command(
    seq_dir: &Path,
    pattern: &str,
    out: &Path,
    o: &Options,
) -> Result<Vec<String>> {
    if o.colour == OutputSpace::Linear {
        bail!("linear output is for TIFF hand-off only; choose rec709, display-p3, rec2020, hdr-pq or hdr-hlg for video");
    }
    if o.colour.is_hdr() && !o.codec.supports_hdr() {
        bail!(
            "{:?} cannot carry HDR; use hevc, av1, prores or dnxhr-hqx/444",
            o.codec
        );
    }
    let (prim, trc, matrix) = o.colour.ffmpeg_tags();
    let mut a: Vec<String> = vec![
        o.ffmpeg.clone(),
        "-hide_banner".into(),
        "-loglevel".into(),
        "warning".into(),
        "-stats".into(),
        "-y".into(),
    ];
    a.extend(["-framerate".into(), format!("{}", o.fps)]);
    a.extend([
        "-i".into(),
        seq_dir.join(pattern).to_string_lossy().into_owned(),
    ]);

    let codec_args = o.codec.args(o.quality);
    let pix_fmt = codec_args
        .iter()
        .skip_while(|x| *x != "-pix_fmt")
        .nth(1)
        .cloned()
        .unwrap_or_default();
    // Convert to the encoder's YUV format first: tmix does not accept 16-bit RGB,
    // and blending planar YUV uses far less memory at 8K.
    let mut vf = vec![
        format!("scale=out_color_matrix={matrix}:out_range=tv:flags=accurate_rnd+full_chroma_int"),
        format!("format={pix_fmt}"),
    ];
    if o.motion_blur > 1 {
        vf.push(format!("tmix=frames={}", o.motion_blur));
    }
    a.extend(["-vf".into(), vf.join(",")]);
    a.extend(codec_args);
    a.extend([
        "-color_primaries".into(),
        prim.into(),
        "-color_trc".into(),
        trc.into(),
    ]);
    a.extend([
        "-colorspace".into(),
        matrix.into(),
        "-color_range".into(),
        "tv".into(),
    ]);

    if o.codec == Codec::Hevc {
        let mut x = format!("colorprim={prim}:transfer={trc}:colormatrix={matrix}:range=limited");
        if o.large_frame {
            // Above 4K, x265's default threading holds many full frames in memory.
            x.push_str(":frame-threads=1:rc-lookahead=10:lookahead-slices=0");
        }
        if o.colour == OutputSpace::HdrPq {
            // BT.2020 primaries, D65, 1000/0.0001 nit mastering display.
            x.push_str(":hdr10=1:repeat-headers=1");
            x.push_str(":master-display=G(8500,39850)B(6550,2300)R(35400,14600)WP(15635,16450)L(10000000,1)");
            x.push_str(&format!(":max-cll={},{}", o.max_cll.0, o.max_cll.1));
        }
        a.extend(["-x265-params".into(), x]);
    }
    if out.extension().and_then(|e| e.to_str()) == Some("mp4") {
        a.extend(["-movflags".into(), "+faststart".into()]);
    }
    a.push(out.to_string_lossy().into_owned());
    Ok(a)
}

pub fn encode(seq_dir: &Path, pattern: &str, out: &Path, o: &Options) -> Result<PathBuf> {
    let args = build_command(seq_dir, pattern, out, o)?;
    if let Some(d) = out.parent() {
        if !d.as_os_str().is_empty() {
            std::fs::create_dir_all(d)?;
        }
    }
    let status = Command::new(&args[0])
        .args(&args[1..])
        .status()
        .with_context(|| {
            format!(
                "could not run `{}` (install ffmpeg and make sure it is on PATH)",
                args[0]
            )
        })?;
    if !status.success() {
        bail!("ffmpeg failed ({status})");
    }
    Ok(out.to_path_buf())
}

pub fn ffmpeg_has_encoder(ffmpeg: &str, encoder: &str) -> bool {
    Command::new(ffmpeg)
        .args(["-hide_banner", "-encoders"])
        .output()
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .split_whitespace()
                .any(|w| w == encoder)
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts(codec: Codec, colour: OutputSpace) -> Options {
        Options {
            codec,
            fps: 25.0,
            quality: 16,
            colour,
            motion_blur: 1,
            max_cll: (1000, 400),
            ffmpeg: "ffmpeg".into(),
            large_frame: false,
        }
    }

    #[test]
    fn hdr10_hevc_is_tagged() {
        let a = build_command(
            Path::new("seq"),
            "frame_%06d.tif",
            Path::new("o.mp4"),
            &opts(Codec::Hevc, OutputSpace::HdrPq),
        )
        .unwrap()
        .join(" ");
        assert!(a.contains("-color_trc smpte2084"));
        assert!(a.contains("hdr10=1"));
        assert!(a.contains("yuv420p10le"));
    }

    #[test]
    fn h264_refuses_hdr() {
        assert!(build_command(
            Path::new("s"),
            "f",
            Path::new("o.mp4"),
            &opts(Codec::H264, OutputSpace::HdrHlg)
        )
        .is_err());
    }
}

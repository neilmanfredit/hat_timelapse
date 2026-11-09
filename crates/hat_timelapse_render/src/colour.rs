//! Colour management for the output sequence.
//!
//! Inputs are either display-referred sRGB (8/16-bit exports) or scene-linear
//! Rec. 709 primaries (32-bit float TIFF, the HDR route out of LightCraft).
//! Everything is converted to linear light, moved to the output primaries and
//! encoded with the output transfer function. The video encoder then only has to
//! convert RGB to YUV and tag the stream, so what you graded is what plays.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum InputSpace {
    /// sRGB / Rec. 709 primaries, sRGB transfer curve (normal 8/16-bit exports).
    Srgb,
    /// Linear light, Rec. 709 primaries, 1.0 = diffuse white (float TIFF).
    LinearRec709,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OutputSpace {
    /// SDR HD/UHD standard. Stills curve kept, stream tagged BT.709.
    Rec709,
    /// SDR wide gamut for Apple displays.
    DisplayP3,
    /// SDR with BT.2020 primaries.
    Rec2020,
    /// HDR10: BT.2020 primaries, SMPTE ST 2084 (PQ). Diffuse white at 203 nits.
    HdrPq,
    /// HDR broadcast: BT.2020 primaries, ARIB STD-B67 (HLG). Diffuse white at 75%.
    HdrHlg,
    /// Scene-linear Rec. 709 float, for compositing in EffectCraft. TIFF only.
    Linear,
}

impl OutputSpace {
    pub fn is_hdr(self) -> bool {
        matches!(self, OutputSpace::HdrPq | OutputSpace::HdrHlg)
    }

    /// ffmpeg tags: (color_primaries, color_trc, colorspace matrix).
    pub fn ffmpeg_tags(self) -> (&'static str, &'static str, &'static str) {
        match self {
            OutputSpace::Rec709 | OutputSpace::Linear => ("bt709", "bt709", "bt709"),
            OutputSpace::DisplayP3 => ("smpte432", "iec61966-2-1", "bt709"),
            OutputSpace::Rec2020 => ("bt2020", "bt2020-10", "bt2020nc"),
            OutputSpace::HdrPq => ("bt2020", "smpte2084", "bt2020nc"),
            OutputSpace::HdrHlg => ("bt2020", "arib-std-b67", "bt2020nc"),
        }
    }
}

/// Reference (diffuse) white for PQ, per ITU-R BT.2408.
pub const PQ_REFERENCE_WHITE_NITS: f32 = 203.0;

const M709_TO_2020: [[f32; 3]; 3] = [
    [0.627_404, 0.329_283, 0.043_313],
    [0.069_097, 0.919_540, 0.011_362],
    [0.016_391, 0.088_013, 0.895_595],
];
const M709_TO_P3: [[f32; 3]; 3] = [
    [0.822_462, 0.177_538, 0.0],
    [0.033_194, 0.966_806, 0.0],
    [0.017_083, 0.072_397, 0.910_520],
];

pub fn srgb_decode(v: f32) -> f32 {
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

pub fn srgb_encode(v: f32) -> f32 {
    let v = v.clamp(0.0, 1.0);
    if v <= 0.003_130_8 {
        v * 12.92
    } else {
        1.055 * v.powf(1.0 / 2.4) - 0.055
    }
}

/// SMPTE ST 2084 inverse EOTF. `nits` absolute luminance.
pub fn pq_encode(nits: f32) -> f32 {
    let (m1, m2) = (0.159_301_76_f32, 78.843_75_f32);
    let (c1, c2, c3) = (0.835_937_5_f32, 18.851_562_f32, 18.687_5_f32);
    let y = (nits / 10_000.0).clamp(0.0, 1.0).powf(m1);
    ((c1 + c2 * y) / (1.0 + c3 * y)).powf(m2)
}

/// ARIB STD-B67 OETF, scene light 0..1.
pub fn hlg_encode(e: f32) -> f32 {
    let (a, b, c) = (0.178_832_77_f32, 0.284_668_92_f32, 0.559_910_7_f32);
    let e = e.max(0.0);
    if e <= 1.0 / 12.0 {
        (3.0 * e).sqrt()
    } else {
        (a * (12.0 * e - b).ln() + c).min(1.0)
    }
}

/// Linear scene value that HLG encodes to 0.75 (reference white).
const HLG_WHITE: f32 = 0.265_1;

fn mul(m: &[[f32; 3]; 3], p: [f32; 3]) -> [f32; 3] {
    [
        m[0][0] * p[0] + m[0][1] * p[1] + m[0][2] * p[2],
        m[1][0] * p[0] + m[1][1] * p[1] + m[1][2] * p[2],
        m[2][0] * p[0] + m[2][1] * p[1] + m[2][2] * p[2],
    ]
}

/// Convert one pixel. Input values are 0..1 for SDR sources, unbounded for linear.
pub fn convert(p: [f32; 3], input: InputSpace, output: OutputSpace) -> [f32; 3] {
    if input == InputSpace::Srgb && output == OutputSpace::Rec709 {
        return p; // keep the exact values the stills editor produced
    }
    let lin = match input {
        InputSpace::Srgb => p.map(srgb_decode),
        InputSpace::LinearRec709 => p.map(|v| v.max(0.0)),
    };
    match output {
        OutputSpace::Rec709 => lin.map(srgb_encode),
        OutputSpace::Linear => lin,
        OutputSpace::DisplayP3 => mul(&M709_TO_P3, lin).map(srgb_encode),
        OutputSpace::Rec2020 => mul(&M709_TO_2020, lin).map(srgb_encode),
        OutputSpace::HdrPq => {
            mul(&M709_TO_2020, lin).map(|v| pq_encode(v * PQ_REFERENCE_WHITE_NITS))
        }
        OutputSpace::HdrHlg => mul(&M709_TO_2020, lin).map(|v| hlg_encode(v * HLG_WHITE)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transfer_functions() {
        assert!((pq_encode(10_000.0) - 1.0).abs() < 1e-4);
        assert!((pq_encode(100.0) - 0.508).abs() < 1e-3);
        assert!((pq_encode(203.0) - 0.581).abs() < 1e-3);
        assert!((hlg_encode(HLG_WHITE) - 0.75).abs() < 1e-3);
        assert!((srgb_decode(srgb_encode(0.18)) - 0.18).abs() < 1e-5);
    }

    #[test]
    fn white_stays_white() {
        for out in [OutputSpace::DisplayP3, OutputSpace::Rec2020] {
            let w = convert([1.0, 1.0, 1.0], InputSpace::Srgb, out);
            assert!(w.iter().all(|v| (v - 1.0).abs() < 1e-3), "{out:?} {w:?}");
        }
    }
}

//! Develop parameters Hat Timelapse can keyframe.
//!
//! Each parameter is stored in XMP under its Camera Raw (`crs:`) name, which both
//! Lightroom and LightCraft read. `lightcraft` is the control name passed to
//! `lightcraft-cli render --set`. The names match LightCraft's own Camera Raw
//! mapping (LightCraft 0.20) and can be overridden per project.

#[derive(Debug, Clone, Copy)]
pub struct ParamDef {
    /// Camera Raw XMP property name, without the `crs:` prefix.
    pub crs: &'static str,
    /// LightCraft develop control, if it has a direct equivalent.
    pub lightcraft: Option<&'static str>,
    pub min: f64,
    pub max: f64,
    pub decimals: usize,
    /// Written with an explicit sign (`+0.50`), as Lightroom does.
    pub signed: bool,
    /// Value assumed when a keyframe does not carry the parameter.
    pub default: Option<f64>,
}

const fn p(
    crs: &'static str,
    lightcraft: Option<&'static str>,
    min: f64,
    max: f64,
    decimals: usize,
    signed: bool,
    default: Option<f64>,
) -> ParamDef {
    ParamDef {
        crs,
        lightcraft,
        min,
        max,
        decimals,
        signed,
        default,
    }
}

pub const EXPOSURE: &str = "Exposure2012";

pub const PARAMS: &[ParamDef] = &[
    p(
        EXPOSURE,
        Some("light.exposure"),
        -5.0,
        5.0,
        2,
        true,
        Some(0.0),
    ),
    p(
        "Contrast2012",
        Some("light.contrast"),
        -100.0,
        100.0,
        0,
        true,
        Some(0.0),
    ),
    p(
        "Highlights2012",
        Some("light.highlights"),
        -100.0,
        100.0,
        0,
        true,
        Some(0.0),
    ),
    p(
        "Shadows2012",
        Some("light.shadows"),
        -100.0,
        100.0,
        0,
        true,
        Some(0.0),
    ),
    p(
        "Whites2012",
        Some("light.whites"),
        -100.0,
        100.0,
        0,
        true,
        Some(0.0),
    ),
    p(
        "Blacks2012",
        Some("light.blacks"),
        -100.0,
        100.0,
        0,
        true,
        Some(0.0),
    ),
    p(
        "Temperature",
        Some("wb.temp"),
        2000.0,
        50000.0,
        0,
        false,
        None,
    ),
    p("Tint", Some("wb.tint"), -150.0, 150.0, 0, true, None),
    p(
        "Texture",
        Some("effects.texture"),
        -100.0,
        100.0,
        0,
        true,
        Some(0.0),
    ),
    p(
        "Clarity2012",
        Some("effects.clarity"),
        -100.0,
        100.0,
        0,
        true,
        Some(0.0),
    ),
    p(
        "Dehaze",
        Some("effects.dehaze"),
        -100.0,
        100.0,
        0,
        true,
        Some(0.0),
    ),
    p(
        "Vibrance",
        Some("color.vibrance"),
        -100.0,
        100.0,
        0,
        true,
        Some(0.0),
    ),
    p(
        "Saturation",
        Some("color.saturation"),
        -100.0,
        100.0,
        0,
        true,
        Some(0.0),
    ),
    // Crop rectangle (normalised 0..1) and angle: keyframing these gives pan and zoom
    // inside Lightroom. LightCraft framing is done by Hat Timelapse's own framing stage.
    p("CropTop", None, 0.0, 1.0, 6, false, None),
    p("CropLeft", None, 0.0, 1.0, 6, false, None),
    p("CropBottom", None, 0.0, 1.0, 6, false, None),
    p("CropRight", None, 0.0, 1.0, 6, false, None),
    p("CropAngle", None, -45.0, 45.0, 2, true, None),
];

pub fn find(crs: &str) -> Option<&'static ParamDef> {
    PARAMS.iter().find(|d| d.crs == crs)
}

impl ParamDef {
    pub fn clamp(&self, v: f64) -> f64 {
        v.clamp(self.min, self.max)
    }

    /// Format a value the way Lightroom writes it into XMP.
    pub fn format(&self, v: f64) -> String {
        let v = self.clamp(v);
        // Avoid "-0" / "+-0.00".
        let scale = 10f64.powi(self.decimals as i32);
        let v = (v * scale).round() / scale;
        let v = if v == 0.0 { 0.0 } else { v };
        if self.signed {
            format!("{:+.*}", self.decimals, v)
        } else {
            format!("{:.*}", self.decimals, v)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_like_lightroom() {
        let e = find(EXPOSURE).unwrap();
        assert_eq!(e.format(0.5), "+0.50");
        assert_eq!(e.format(-0.004), "+0.00");
        assert_eq!(e.format(9.0), "+5.00");
        assert_eq!(find("Temperature").unwrap().format(5500.4), "5500");
        assert_eq!(find("Contrast2012").unwrap().format(-12.4), "-12");
    }
}

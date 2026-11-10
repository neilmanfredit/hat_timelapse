//! The project file (`hat_timelapse.json`) that lives beside the images.

use crate::exif::ExposureInfo;
use crate::interp::Method;
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub const PROJECT_FILE: &str = "hat_timelapse.json";
pub const WORK_DIR: &str = ".hat_timelapse";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Frame {
    /// File name relative to the project folder.
    pub file: String,
    #[serde(default)]
    pub exif: ExposureInfo,
    #[serde(default)]
    pub keyframe: bool,
    /// Removed from the render by a long-term filter or by hand.
    #[serde(default)]
    pub excluded: bool,
    /// Exposure compensation for camera setting changes (EV).
    #[serde(default)]
    pub holy_grail_ev: f64,
    /// Exposure correction from deflicker (EV).
    #[serde(default)]
    pub deflicker_ev: f64,
    /// Exposure offset last written into this frame's sidecar on top of the
    /// user's keyframe exposure. Subtracted again when keyframes are re-read.
    #[serde(default)]
    pub applied_offset_ev: f64,
    /// Measured log2 mean luminance of the source image (before Hat Timelapse's offsets).
    #[serde(default)]
    pub luminance: Option<f64>,
    /// Interpolated develop values (crs name → value), excluding Hat Timelapse offsets.
    #[serde(default)]
    pub values: BTreeMap<String, f64>,
    /// The exposure value last written into this frame's sidecar.
    #[serde(default)]
    pub written_exposure: Option<f64>,
    /// Fingerprint of the sidecar as Hat Timelapse last wrote it. A different
    /// fingerprint means an editor has saved the file since.
    #[serde(default)]
    pub sidecar_fp: Option<u64>,
    /// An editor (Lightroom, LightCraft) has saved edits to this frame.
    #[serde(default)]
    pub editor_saved: bool,
}

impl Frame {
    pub fn new(file: String, exif: ExposureInfo) -> Self {
        Frame {
            file,
            exif,
            keyframe: false,
            excluded: false,
            holy_grail_ev: 0.0,
            deflicker_ev: 0.0,
            applied_offset_ev: 0.0,
            written_exposure: None,
            sidecar_fp: None,
            editor_saved: false,
            luminance: None,
            values: BTreeMap::new(),
        }
    }

    pub fn offset_ev(&self) -> f64 {
        self.holy_grail_ev + self.deflicker_ev
    }
}

/// A pan/zoom keyframe for the framing stage.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct MotionKey {
    pub frame: usize,
    /// 1.0 = the full output-aspect crop of the frame; 2.0 = twice as close.
    pub zoom: f64,
    /// Centre of the view, normalised 0..1.
    pub cx: f64,
    pub cy: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default)]
    pub interpolation: Method,
    /// Argument template for rendering one frame with LightCraft.
    /// `{input}`, `{output}`, `{xmp}` (the frame's sidecar), `{bitdepth}` and
    /// `{sets}` (`--set control=value` pairs) are substituted.
    #[serde(default = "default_lightcraft_cmd")]
    pub lightcraft_command: Vec<String>,
    /// Override the crs → LightCraft control mapping.
    #[serde(default)]
    pub lightcraft_controls: BTreeMap<String, String>,
    #[serde(default = "default_fps")]
    pub fps: f64,
}

fn default_lightcraft_cmd() -> Vec<String> {
    [
        "lightcraft-cli",
        "render",
        "{input}",
        "-o",
        "{output}",
        "--opt",
        "bitDepth={bitdepth}",
        "--settings",
        "{xmp}",
        "{sets}",
    ]
    .map(String::from)
    .to_vec()
}

fn default_fps() -> f64 {
    25.0
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            interpolation: Method::default(),
            lightcraft_command: default_lightcraft_cmd(),
            lightcraft_controls: BTreeMap::new(),
            fps: default_fps(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub format: u32,
    #[serde(skip)]
    pub dir: PathBuf,
    pub frames: Vec<Frame>,
    #[serde(default)]
    pub motion: Vec<MotionKey>,
    /// Develop settings that are not animated (tone curve, HSL, lens
    /// corrections, profile…), taken from the first keyframe and written to
    /// every frame so the whole sequence matches it.
    #[serde(default)]
    pub shared: SharedSettings,
    #[serde(default)]
    pub settings: Settings,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SharedSettings {
    /// Frame the settings were taken from.
    pub source_frame: Option<usize>,
    /// Simple properties, qualified name → value.
    pub properties: BTreeMap<String, String>,
    /// Structured properties as raw XMP, qualified name → XML.
    pub elements: BTreeMap<String, String>,
    /// Namespaces those properties use, prefix → URI.
    pub namespaces: BTreeMap<String, String>,
    /// LightCraft's complete develop settings (`lc:settings`) from the source
    /// keyframe, when it was edited in LightCraft.
    #[serde(default)]
    pub lightcraft: Option<serde_json::Value>,
}

impl Project {
    pub fn new(dir: &Path, frames: Vec<Frame>) -> Self {
        Project {
            format: 1,
            dir: dir.to_path_buf(),
            frames,
            motion: Vec::new(),
            shared: SharedSettings::default(),
            settings: Settings::default(),
        }
    }

    pub fn file_path(dir: &Path) -> PathBuf {
        dir.join(PROJECT_FILE)
    }

    pub fn work_dir(&self) -> PathBuf {
        self.dir.join(WORK_DIR)
    }

    pub fn load(dir: &Path) -> Result<Self> {
        let path = Self::file_path(dir);
        let text = std::fs::read_to_string(&path).with_context(|| {
            format!(
                "no project at {} (run `hat_timelapse init` first)",
                path.display()
            )
        })?;
        let mut p: Project =
            serde_json::from_str(&text).with_context(|| format!("reading {}", path.display()))?;
        if p.format != 1 {
            bail!("unsupported project format {}", p.format);
        }
        p.dir = dir.to_path_buf();
        Ok(p)
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::file_path(&self.dir);
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_string_pretty(self)?)?;
        std::fs::rename(&tmp, &path)?;
        Ok(())
    }

    pub fn frame_path(&self, i: usize) -> PathBuf {
        self.dir.join(&self.frames[i].file)
    }

    pub fn keyframe_indices(&self) -> Vec<usize> {
        self.frames
            .iter()
            .enumerate()
            .filter(|(_, f)| f.keyframe)
            .map(|(i, _)| i)
            .collect()
    }

    pub fn included_indices(&self) -> Vec<usize> {
        self.frames
            .iter()
            .enumerate()
            .filter(|(_, f)| !f.excluded)
            .map(|(i, _)| i)
            .collect()
    }

    /// Mark `count` evenly spaced keyframes, always including first and last.
    pub fn auto_keyframes(&mut self, count: usize) {
        let n = self.frames.len();
        for f in &mut self.frames {
            f.keyframe = false;
        }
        if n == 0 {
            return;
        }
        let count = count.clamp(1, n);
        for k in 0..count {
            let i = if count == 1 {
                0
            } else {
                (k as f64 * (n - 1) as f64 / (count - 1) as f64).round() as usize
            };
            self.frames[i].keyframe = true;
        }
    }

    /// Final value for a parameter on a frame, including Hat Timelapse's exposure offsets.
    pub fn final_value(&self, i: usize, crs: &str) -> Option<f64> {
        let f = &self.frames[i];
        let base = f.values.get(crs).copied();
        if crs == crate::params::EXPOSURE {
            Some(base.unwrap_or(0.0) + f.offset_ev())
        } else {
            base
        }
    }

    /// Interpolated pan/zoom for every frame.
    pub fn motion_track(&self) -> Vec<MotionKey> {
        let n = self.frames.len();
        if self.motion.is_empty() {
            return (0..n)
                .map(|i| MotionKey {
                    frame: i,
                    zoom: 1.0,
                    cx: 0.5,
                    cy: 0.5,
                })
                .collect();
        }
        let mut keys = self.motion.clone();
        keys.sort_by_key(|k| k.frame);
        keys.dedup_by_key(|k| k.frame);
        let m = self.settings.interpolation;
        let pts = |f: fn(&MotionKey) -> f64| {
            keys.iter()
                .map(|k| (k.frame as f64, f(k)))
                .collect::<Vec<_>>()
        };
        let z = crate::interp::interpolate(&pts(|k| k.zoom), n, m);
        let x = crate::interp::interpolate(&pts(|k| k.cx), n, m);
        let y = crate::interp::interpolate(&pts(|k| k.cy), n, m);
        (0..n)
            .map(|i| MotionKey {
                frame: i,
                zoom: z[i],
                cx: x[i],
                cy: y[i],
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_keyframes_spread() {
        let frames = (0..10)
            .map(|i| Frame::new(format!("{i}.jpg"), Default::default()))
            .collect();
        let mut p = Project::new(Path::new("."), frames);
        p.auto_keyframes(4);
        assert_eq!(p.keyframe_indices(), vec![0, 3, 6, 9]);
    }
}

//! Developing frames with LightCraft's command-line renderer.

use anyhow::{bail, Context, Result};
use rayon::prelude::*;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Debug, Clone)]
pub struct Job {
    pub input: PathBuf,
    pub output: PathBuf,
    /// (LightCraft control, value)
    pub sets: Vec<(String, f64)>,
}

/// Expand the argument template. `{sets}` becomes `--set control=value` pairs.
pub fn build_args(template: &[String], job: &Job) -> Vec<String> {
    let mut out = Vec::new();
    for t in template {
        if t == "{sets}" {
            for (k, v) in &job.sets {
                out.push("--set".into());
                out.push(format!("{k}={v:.4}"));
            }
        } else {
            out.push(
                t.replace("{input}", &job.input.to_string_lossy())
                    .replace("{output}", &job.output.to_string_lossy()),
            );
        }
    }
    out
}

pub fn check_available(program: &str) -> bool {
    Command::new(program)
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Render all jobs, skipping outputs that already exist unless `overwrite`.
/// Progress is reported through `progress(done, total)`.
pub fn render(
    template: &[String],
    jobs: &[Job],
    overwrite: bool,
    progress: impl Fn(usize, usize) + Sync,
) -> Result<usize> {
    let Some(program) = template.first() else {
        bail!("empty LightCraft command template")
    };
    let done = AtomicUsize::new(0);
    let rendered = AtomicUsize::new(0);
    jobs.par_iter().try_for_each(|job| -> Result<()> {
        if overwrite || !job.output.exists() {
            if let Some(d) = job.output.parent() {
                std::fs::create_dir_all(d)?;
            }
            let args = build_args(template, job);
            let out = Command::new(program)
                .args(&args[1..])
                .output()
                .with_context(|| {
                    format!(
                        "could not run `{program}` (is LightCraft's CLI installed and on PATH?)"
                    )
                })?;
            if !out.status.success() || !job.output.exists() {
                bail!(
                    "LightCraft failed on {}: {}",
                    job.input.display(),
                    String::from_utf8_lossy(&out.stderr).trim()
                );
            }
            rendered.fetch_add(1, Ordering::Relaxed);
        }
        progress(done.fetch_add(1, Ordering::Relaxed) + 1, jobs.len());
        Ok(())
    })?;
    Ok(rendered.into_inner())
}

pub fn rendered_name(input: &Path, ext: &str) -> String {
    format!("{}.{ext}", input.file_stem().unwrap().to_string_lossy())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn template_expansion() {
        let t: Vec<String> = [
            "lightcraft-cli",
            "render",
            "{input}",
            "-o",
            "{output}",
            "{sets}",
        ]
        .map(String::from)
        .to_vec();
        let j = Job {
            input: "a.dng".into(),
            output: "a.tif".into(),
            sets: vec![("light.exposure".into(), 0.5)],
        };
        assert_eq!(
            build_args(&t, &j),
            [
                "lightcraft-cli",
                "render",
                "a.dng",
                "-o",
                "a.tif",
                "--set",
                "light.exposure=0.5000"
            ]
        );
    }
}

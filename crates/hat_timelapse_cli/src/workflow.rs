//! The steps behind each command, shared by single-project commands and batch.

use crate::{BatchArgs, DeflickerArgs, ExportArgs, FrameArgs, RenderArgs, SourceArg, VideoArgs};
use anyhow::{anyhow, bail, Context, Result};
use hat_timelapse_core::deflicker::{self, Source};
use hat_timelapse_core::interp::{interpolate as interp, Method};
use hat_timelapse_core::luminance::{self, Roi};
use hat_timelapse_core::params::{self, EXPOSURE, PARAMS};
use hat_timelapse_core::project::{Frame, Project, PROJECT_FILE};
use hat_timelapse_core::{exif, holygrail, sequence, MotionKey};
use hat_timelapse_render::colour::OutputSpace;
use hat_timelapse_render::framing::{self, FrameJob, Size};
use hat_timelapse_render::{lightcraft, video};
use hat_timelapse_xmp::Edit;
use rayon::prelude::*;
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

pub fn load(dir: &Path) -> Result<Project> {
    Project::load(dir)
}

pub fn init(dir: &Path, force: bool, keyframes: usize) -> Result<Project> {
    if Project::file_path(dir).exists() && !force {
        bail!(
            "{} already has a project (use --force to start again)",
            dir.display()
        );
    }
    let files = sequence::scan(dir).with_context(|| format!("reading {}", dir.display()))?;
    if files.is_empty() {
        bail!("no images found in {}", dir.display());
    }
    eprintln!("reading metadata from {} images…", files.len());
    let frames: Vec<Frame> = files
        .par_iter()
        .map(|f| {
            Frame::new(
                f.file_name().unwrap().to_string_lossy().into_owned(),
                exif::read(f),
            )
        })
        .collect();
    let mut p = Project::new(dir, frames);
    p.auto_keyframes(keyframes);
    p.save()?;
    let raw = files.iter().filter(|f| sequence::is_raw(f)).count();
    let with_ev = p.frames.iter().filter(|f| f.exif.ev().is_some()).count();
    println!(
        "{}: {} frames ({raw} raw), exposure metadata on {with_ev}",
        PROJECT_FILE,
        p.frames.len()
    );
    if raw < files.len() {
        println!(
            "note: Lightroom ignores .xmp sidecars for JPEG, TIFF and DNG; after `hat_timelapse write`, use \
             Metadata > Read Metadata from Files there. LightCraft reads sidecars for every format."
        );
    }
    Ok(p)
}

pub fn status(dir: &Path) -> Result<()> {
    let p = load(dir)?;
    let n = p.frames.len();
    let inc = p.included_indices().len();
    let evs: Vec<f64> = p.frames.iter().filter_map(|f| f.exif.ev()).collect();
    println!("project     {}", dir.display());
    println!("frames      {n} ({inc} included, {} excluded)", n - inc);
    println!("keyframes   {:?}", p.keyframe_indices());
    if let (Some(a), Some(b)) = (
        evs.iter().cloned().reduce(f64::min),
        evs.iter().cloned().reduce(f64::max),
    ) {
        println!("camera EV   {a:.2} .. {b:.2} ({:.2} stops of range)", b - a);
    }
    let first = p
        .frames
        .first()
        .and_then(|f| f.exif.captured.clone())
        .unwrap_or_default();
    let last = p
        .frames
        .last()
        .and_then(|f| f.exif.captured.clone())
        .unwrap_or_default();
    println!("captured    {first} → {last}");
    let hg = p
        .frames
        .iter()
        .map(|f| f.holy_grail_ev.abs())
        .fold(0.0, f64::max);
    let df = p
        .frames
        .iter()
        .map(|f| f.deflicker_ev.abs())
        .fold(0.0, f64::max);
    println!("holy grail  max offset {hg:.2} EV");
    println!("deflicker   max correction {df:.2} EV");
    let interpolated: Vec<&String> = p
        .frames
        .first()
        .map(|f| f.values.keys().collect())
        .unwrap_or_default();
    println!("animated    {interpolated:?}");
    println!("motion keys {}", p.motion.len());
    println!("fps         {}", p.settings.fps);
    Ok(())
}

pub fn label_keyframes(p: &Project) -> Result<()> {
    for (i, f) in p.frames.iter().enumerate() {
        let path = sequence::sidecar_path(&p.frame_path(i));
        if f.keyframe {
            hat_timelapse_xmp::update_file(&path, &Edit::default().set("xmp:Label", "Keyframe"))?;
        } else if path.exists() {
            let props = hat_timelapse_xmp::read_file(&path)?;
            if props.get("xmp:Label").map(String::as_str) == Some("Keyframe") {
                hat_timelapse_xmp::update_file(&path, &Edit::default().remove("xmp:Label"))?;
            }
        }
    }
    println!(
        "labelled {} keyframes as \"Keyframe\"",
        p.keyframe_indices().len()
    );
    Ok(())
}

pub fn holy_grail(dir: &Path, reference: usize, threshold: f64) -> Result<()> {
    let mut p = load(dir)?;
    let r = holygrail::apply(&mut p, reference, threshold);
    println!("{} exposure changes found", r.steps.len());
    for s in r.steps.iter().take(20) {
        println!("  frame {:6}  {:+.2} EV", s.frame, s.delta_ev);
    }
    if r.steps.len() > 20 {
        println!("  … and {} more", r.steps.len() - 20);
    }
    if r.missing_exif > 0 {
        println!(
            "warning: {} frames had no exposure metadata; nearest values were used",
            r.missing_exif
        );
    }
    println!(
        "compensation range {:+.2} .. {:+.2} EV",
        r.min_offset, r.max_offset
    );
    apply_keyframes(&mut p)?;
    write_sidecars(&mut p)?;
    p.save()
}

/// Read keyframe develop settings from sidecars and interpolate every parameter.
pub fn interpolate(dir: &Path, method: Option<Method>) -> Result<()> {
    let mut p = load(dir)?;
    if let Some(m) = method {
        p.settings.interpolation = m;
    }
    if p.keyframe_indices().is_empty() {
        bail!("no keyframes marked (use `hat_timelapse keyframes auto 3`)");
    }
    apply_keyframes(&mut p)?;
    write_sidecars(&mut p)?;
    p.save()
}

/// The user's own exposure on a keyframe, from what the editor saved (`v`).
///
/// Hat Timelapse writes exposure as the user's value plus its holy-grail and
/// deflicker offsets. Normally the editor re-reads that before the user edits
/// again, so the offset is subtracted. If the editor saved without re-reading
/// (Lightroom's Save Metadata on a stale catalogue), it writes back the user's
/// previous value unchanged; that case is recognised and not double-counted.
pub fn keyframe_user_exposure(f: &Frame, v: f64) -> f64 {
    const SAME: f64 = 0.006; // half of the 0.01 EV the editors round to
    let previous = f.values.get(EXPOSURE).copied();
    match (f.written_exposure, previous) {
        (Some(w), Some(u)) if (v - w).abs() < SAME => u, // untouched since we wrote it
        (Some(w), _) if (v - w).abs() < SAME => v - f.applied_offset_ev,
        (Some(_), Some(u)) if (v - u).abs() < SAME && f.applied_offset_ev.abs() >= SAME => u, // stale save
        _ => v - f.applied_offset_ev,
    }
}

/// Read every keyframe's edits from its sidecar and interpolate them across
/// all frames. Run before anything writes sidecars, so steps can be taken in
/// any order without losing keyframe edits.
pub fn apply_keyframes(p: &mut Project) -> Result<()> {
    let keys = p.keyframe_indices();
    if keys.is_empty() {
        return Ok(());
    }
    // Note which keyframes an editor has saved since Hat Timelapse last wrote them.
    let mut newly_edited = 0;
    for &i in &keys {
        let path = sequence::sidecar_path(&p.frame_path(i));
        if let Some(fp) = hat_timelapse_core::status::fingerprint_file(&path) {
            if p.frames[i].sidecar_fp != Some(fp) {
                p.frames[i].editor_saved = true;
                newly_edited += 1;
            }
        }
    }
    let props: Vec<_> = keys
        .iter()
        .map(|&i| {
            let path = sequence::sidecar_path(&p.frame_path(i));
            if path.exists() {
                hat_timelapse_xmp::read_file(&path)
            } else {
                Ok(Default::default())
            }
        })
        .collect::<Result<_>>()?;
    // Keyframes edited in LightCraft carry their values in `lc:settings`, which
    // LightCraft prefers over the Camera Raw fields. Read those where present.
    let props: Vec<BTreeMap<String, String>> = props
        .into_iter()
        .map(|pr| with_lightcraft_values(p, pr))
        .collect();

    let n = p.frames.len();
    let mut animated = Vec::new();
    for def in PARAMS {
        let qn = format!("crs:{}", def.crs);
        let mut pts = Vec::new();
        let mut missing = 0;
        for (&i, pr) in keys.iter().zip(&props) {
            let v = pr
                .get(&qn)
                .and_then(|s| s.trim_start_matches('+').parse::<f64>().ok());
            let v = match v {
                Some(v) if def.crs == EXPOSURE => Some(keyframe_user_exposure(&p.frames[i], v)),
                other => other,
            };
            match v.or(def.default) {
                Some(v) => pts.push((i as f64, v)),
                None => missing += 1,
            }
        }
        let seen = props.iter().any(|pr| pr.contains_key(&qn));
        if !seen {
            for f in &mut p.frames {
                f.values.remove(def.crs);
            }
            continue;
        }
        if missing > 0 {
            eprintln!(
                "warning: {} is set on some keyframes but not others; not animated",
                def.crs
            );
            continue;
        }
        let vals = interp(&pts, n, p.settings.interpolation);
        for (f, v) in p.frames.iter_mut().zip(vals) {
            f.values.insert(def.crs.to_string(), v);
        }
        animated.push(def.crs);
    }
    println!(
        "applied edits from {} keyframes ({newly_edited} saved since last time) to {n} frames: {animated:?}",
        keys.len()
    );
    p.shared = shared_settings(p, &keys, &animated)?;
    Ok(())
}

/// LightCraft control path for a parameter, honouring project overrides.
fn lc_control<'a>(p: &'a Project, def: &'a params::ParamDef) -> Option<&'a str> {
    p.settings
        .lightcraft_controls
        .get(def.crs)
        .map(String::as_str)
        .or(def.lightcraft)
}

fn json_get(v: &serde_json::Value, path: &str) -> Option<f64> {
    path.split('.').try_fold(v, |v, k| v.get(k))?.as_f64()
}

fn json_set(v: &mut serde_json::Value, path: &str, x: f64) {
    let mut cur = v;
    let parts: Vec<&str> = path.split('.').collect();
    for (k, part) in parts.iter().enumerate() {
        if !cur.is_object() {
            *cur = serde_json::json!({});
        }
        let obj = cur.as_object_mut().expect("object");
        if k == parts.len() - 1 {
            obj.insert(part.to_string(), serde_json::json!(x));
            return;
        }
        cur = obj
            .entry(part.to_string())
            .or_insert_with(|| serde_json::json!({}));
    }
}

/// Overlay values from a sidecar's `lc:settings` onto its Camera Raw properties.
/// Values at their neutral default are skipped (LightCraft writes every field),
/// and white balance counts only when it is set to a custom value.
fn with_lightcraft_values(
    p: &Project,
    mut props: BTreeMap<String, String>,
) -> BTreeMap<String, String> {
    let Some(json) = props
        .get("lc:settings")
        .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok())
    else {
        return props;
    };
    let custom_wb = json.pointer("/wb/mode").and_then(|m| m.as_str()) == Some("custom");
    for def in PARAMS {
        let Some(path) = lc_control(p, def) else {
            continue;
        };
        let qn = format!("crs:{}", def.crs);
        let Some(v) = json_get(&json, path) else {
            props.remove(&qn);
            continue;
        };
        let is_wb = path.starts_with("wb.");
        if (is_wb && !custom_wb) || (!is_wb && def.default == Some(v)) {
            props.remove(&qn);
        } else {
            props.insert(qn, v.to_string());
        }
    }
    props
}

/// A frame's complete LightCraft settings: the source keyframe's settings with
/// this frame's animated values (including exposure offsets) applied.
pub fn frame_lightcraft_json(p: &Project, i: usize) -> Option<serde_json::Value> {
    let mut json = p.shared.lightcraft.clone()?;
    let f = &p.frames[i];
    let mut keys: Vec<&str> = f.values.keys().map(String::as_str).collect();
    if !keys.contains(&EXPOSURE) {
        keys.push(EXPOSURE);
    }
    for k in keys {
        let def = params::find(k).expect("known parameter");
        if let (Some(path), Some(v)) = (lc_control(p, def), p.final_value(i, k)) {
            json_set(&mut json, path, def.clamp(v));
        }
    }
    Some(json)
}

/// Properties that describe a particular file or are managed by Hat Timelapse,
/// and so are never copied between frames.
const NOT_SHARED: &[&str] = &[
    "crs:RawFileName",
    "crs:AlreadyApplied",
    "crs:HasSettings",
    "crs:AsShotTemperature",
    "crs:AsShotTint",
];

/// Collect the first keyframe's non-animated develop settings, and warn about
/// any that other keyframes set differently (those differences are not used).
fn shared_settings(
    p: &Project,
    keys: &[usize],
    animated: &[&str],
) -> Result<hat_timelapse_core::project::SharedSettings> {
    let snap_of = |i: usize| -> Result<Option<hat_timelapse_xmp::Snapshot>> {
        let path = sequence::sidecar_path(&p.frame_path(i));
        if path.exists() {
            hat_timelapse_xmp::snapshot_file(&path).map(Some)
        } else {
            Ok(None)
        }
    };
    let Some(first) = snap_of(keys[0])? else {
        return Ok(Default::default());
    };
    let lightcraft = first
        .properties
        .get("lc:settings")
        .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok())
        .filter(|v| v.is_object());
    let keep = |k: &String| {
        k.starts_with("crs:")
            && !NOT_SHARED.contains(&k.as_str())
            && !animated.iter().any(|a| k.strip_prefix("crs:") == Some(*a))
    };
    let properties: BTreeMap<String, String> = first
        .properties
        .into_iter()
        .filter(|(k, _)| keep(k))
        .collect();
    let elements: BTreeMap<String, String> = first
        .elements
        .into_iter()
        .filter(|(k, _)| keep(k))
        .collect();

    let mut differing = std::collections::BTreeSet::new();
    for &i in &keys[1..] {
        if let Some(other) = snap_of(i)? {
            for (k, v) in &properties {
                if other.properties.get(k).is_some_and(|o| o != v) {
                    differing.insert(k.clone());
                }
            }
            for (k, v) in &elements {
                if other.elements.get(k).is_some_and(|o| o != v) {
                    differing.insert(k.clone());
                }
            }
        }
    }
    if let Some(base) = &lightcraft {
        // Compare LightCraft settings with the animated controls blanked out.
        let blank = |mut v: serde_json::Value| {
            for def in PARAMS {
                if let Some(path) = lc_control(p, def) {
                    json_set(&mut v, path, 0.0);
                }
            }
            v
        };
        let base = blank(base.clone());
        for &i in &keys[1..] {
            let other = snap_of(i)?
                .and_then(|s| s.properties.get("lc:settings").cloned())
                .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok());
            if other.is_some_and(|o| blank(o) != base) {
                differing.insert("LightCraft settings (curve, colour, detail, masks…)".to_string());
                break;
            }
        }
    }
    if !differing.is_empty() {
        eprintln!(
            "warning: these settings differ between keyframes but cannot be animated, so the first \
             keyframe's values are used for every frame: {}",
            differing.iter().map(|k| k.trim_start_matches("crs:")).collect::<Vec<_>>().join(", ")
        );
    }
    println!(
        "copied {} other settings from keyframe {} to every frame",
        properties.len() + elements.len(),
        keys[0]
    );
    Ok(hat_timelapse_core::project::SharedSettings {
        source_frame: Some(keys[0]),
        properties,
        elements,
        namespaces: first.namespaces,
        lightcraft,
    })
}

/// Write the current develop values plus Hat Timelapse's exposure offsets into every sidecar.
pub fn write_sidecars(p: &mut Project) -> Result<()> {
    let clamped = std::sync::atomic::AtomicUsize::new(0);
    let edits: Vec<(PathBuf, Edit, f64)> = (0..p.frames.len())
        .map(|i| {
            let f = &p.frames[i];
            let mut e = Edit {
                elements: p.shared.elements.clone(),
                namespaces: p.shared.namespaces.clone(),
                ..Default::default()
            };
            for (k, v) in &p.shared.properties {
                e = e.set(k, v.clone());
            }
            if let Some(json) = frame_lightcraft_json(p, i) {
                e = e.set("lc:settings", json.to_string());
            }
            let mut e = e
                .set("crs:HasSettings", "True")
                .set_if_absent("crs:ProcessVersion", "11.0")
                .set_if_absent("crs:Version", "15.0");
            let mut keys: Vec<&str> = f.values.keys().map(String::as_str).collect();
            if !keys.contains(&EXPOSURE) {
                keys.push(EXPOSURE);
            }
            for k in keys {
                let def = params::find(k).expect("known parameter");
                let v = p.final_value(i, k).unwrap_or(0.0);
                if v < def.min || v > def.max {
                    clamped.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                }
                e = e.set(&format!("crs:{k}"), def.format(v));
            }
            if f.values.contains_key("CropTop") {
                e = e.set("crs:HasCrop", "True");
            }
            (sequence::sidecar_path(&p.frame_path(i)), e, f.offset_ev())
        })
        .collect();
    edits
        .par_iter()
        .try_for_each(|(path, e, _)| hat_timelapse_xmp::update_file(path, e))?;
    for (f, (path, e, off)) in p.frames.iter_mut().zip(&edits) {
        f.applied_offset_ev = *off;
        f.written_exposure = e
            .set
            .get(&format!("crs:{EXPOSURE}"))
            .and_then(|v| v.trim_start_matches('+').parse().ok());
        f.sidecar_fp = hat_timelapse_core::status::fingerprint_file(path);
    }
    let c = clamped.into_inner();
    if c > 0 {
        eprintln!("warning: {c} values were outside the editor's range and were clamped (e.g. exposure beyond ±5 EV)");
    }
    println!("wrote {} sidecars", edits.len());
    Ok(())
}

fn previews_dir(p: &Project) -> PathBuf {
    p.work_dir().join("previews")
}

fn rendered_dir(p: &Project) -> PathBuf {
    p.work_dir().join("rendered")
}

/// Measure every frame from the originals (cached in the project).
pub fn ensure_luminance(p: &mut Project, roi: Option<Roi>) -> Result<()> {
    let todo: Vec<usize> = (0..p.frames.len())
        .filter(|&i| roi.is_some() || p.frames[i].luminance.is_none())
        .collect();
    if todo.is_empty() {
        return Ok(());
    }
    eprintln!("measuring brightness of {} frames…", todo.len());
    let res: Vec<(usize, Option<f64>)> = todo
        .par_iter()
        .map(|&i| {
            let r = luminance::measure_file(&p.frame_path(i), roi);
            if let Err(e) = &r {
                eprintln!("warning: {e:#}");
            }
            (i, r.ok())
        })
        .collect();
    for (i, l) in res {
        p.frames[i].luminance = l;
    }
    Ok(())
}

pub fn deflicker(dir: &Path, a: &DeflickerArgs) -> Result<()> {
    let mut p = load(dir)?;
    // Measure against the latest keyframe edits.
    apply_keyframes(&mut p)?;
    if a.reset {
        for f in &mut p.frames {
            f.deflicker_ev = 0.0;
        }
    }
    let (measured, source) = match a.source {
        SourceArg::Originals => {
            ensure_luminance(&mut p, a.roi)?;
            (
                p.frames.iter().map(|f| f.luminance).collect::<Vec<_>>(),
                Source::Originals,
            )
        }
        SourceArg::Previews => {
            let pd = previews_dir(&p);
            let map = stem_map(&pd)?;
            if map.is_empty() {
                bail!("no previews in {} (run `hat_timelapse render --previews` or export small JPEGs there from Lightroom)", pd.display());
            }
            let m: Vec<Option<f64>> = p
                .frames
                .par_iter()
                .map(|f| {
                    let stem = Path::new(&f.file)
                        .file_stem()
                        .unwrap()
                        .to_string_lossy()
                        .into_owned();
                    map.get(&stem)
                        .and_then(|path| luminance::measure_file(path, a.roi).ok())
                })
                .collect();
            (m, Source::Previews)
        }
    };
    let measured_count = measured.iter().flatten().count();
    if measured_count < 3 {
        bail!("only {measured_count} frames could be measured; nothing to deflicker");
    }
    let max = deflicker::update(&mut p, &measured, source, a.window);
    println!(
        "deflicker: {measured_count} frames measured, largest correction this pass {max:.3} EV"
    );
    if source == Source::Previews {
        println!("re-render previews and run again until the correction is small (under ~0.02 EV)");
    }
    write_sidecars(&mut p)?;
    p.save()
}

/// Map file stem → path for images in a folder.
fn stem_map(dir: &Path) -> Result<HashMap<String, PathBuf>> {
    if !dir.exists() {
        return Ok(HashMap::new());
    }
    Ok(sequence::scan(dir)?
        .into_iter()
        .map(|p| (p.file_stem().unwrap().to_string_lossy().into_owned(), p))
        .collect())
}

pub fn render(dir: &Path, a: &RenderArgs) -> Result<()> {
    let p = load(dir)?;
    let (out_dir, ext) = if a.previews {
        (previews_dir(&p), "jpg")
    } else {
        (rendered_dir(&p), a.format.as_str())
    };
    let jobs: Vec<lightcraft::Job> = p
        .included_indices()
        .into_iter()
        .map(|i| {
            let input = p.frame_path(i);
            let sets = PARAMS
                .iter()
                .filter_map(|d| {
                    let ctrl = p
                        .settings
                        .lightcraft_controls
                        .get(d.crs)
                        .map(String::as_str)
                        .or(d.lightcraft)?;
                    let v = p.final_value(i, d.crs)?;
                    Some((ctrl.to_string(), d.clamp(v)))
                })
                .collect();
            let output = out_dir.join(lightcraft::rendered_name(&input, ext));
            let (bit_depth, extra) = if a.previews {
                (8, vec!["--size".to_string(), "1024".to_string()])
            } else {
                (a.bit_depth, Vec::new())
            };
            // LightCraft-edited sequences get each frame's complete settings as
            // JSON (merged by `render --settings`); others use the sidecar.
            let settings = match frame_lightcraft_json(&p, i) {
                Some(json) => {
                    let path = p
                        .work_dir()
                        .join("settings")
                        .join(lightcraft::rendered_name(&input, "json"));
                    if let Err(e) = std::fs::create_dir_all(path.parent().unwrap())
                        .and_then(|_| std::fs::write(&path, json.to_string()))
                    {
                        eprintln!("warning: could not write {}: {e}", path.display());
                    }
                    path
                }
                None => sequence::sidecar_path(&input),
            };
            lightcraft::Job {
                xmp: settings,
                input,
                output,
                bit_depth,
                sets,
                extra,
            }
        })
        .collect();
    let n = jobs.len();
    let done = lightcraft::render(
        &p.settings.lightcraft_command,
        &jobs,
        a.overwrite || a.previews,
        |d, t| {
            if d % 10 == 0 || d == t {
                eprint!("\rrendering {d}/{t}");
            }
        },
    )?;
    eprintln!();
    println!("{done} of {n} frames rendered into {}", out_dir.display());
    Ok(())
}

/// Work out the developed image for each included frame, plus its pan/zoom.
/// Without a project, every image in `from` is used in name order.
fn frame_jobs(dir: &Path, from: Option<&Path>) -> Result<(Vec<FrameJob>, PathBuf, String, f64)> {
    match load(dir) {
        Ok(p) => {
            let from = from
                .map(Path::to_path_buf)
                .unwrap_or_else(|| rendered_dir(&p));
            let map = stem_map(&from)?;
            if map.is_empty() {
                bail!("no developed images in {} (run `hat_timelapse render`, or pass --from with your Lightroom export)", from.display());
            }
            let track = p.motion_track();
            let inc = p.included_indices();
            let mut jobs = Vec::new();
            for &i in &inc {
                let stem = Path::new(&p.frames[i].file)
                    .file_stem()
                    .unwrap()
                    .to_string_lossy()
                    .into_owned();
                if let Some(path) = map.get(&stem) {
                    jobs.push(FrameJob {
                        input: path.clone(),
                        motion: track[i],
                    });
                }
            }
            if jobs.is_empty() {
                // Exported with different names: fall back to folder order.
                eprintln!(
                    "note: file names in {} do not match the project; using folder order",
                    from.display()
                );
                let files = sequence::scan(&from)?;
                let n = p.frames.len().max(1);
                jobs = files
                    .into_iter()
                    .enumerate()
                    .map(|(k, f)| FrameJob {
                        input: f,
                        motion: track[k.min(n - 1)],
                    })
                    .collect();
            } else if jobs.len() < inc.len() {
                eprintln!(
                    "warning: {} included frames have no developed image in {}",
                    inc.len() - jobs.len(),
                    from.display()
                );
            }
            let name = dir
                .canonicalize()
                .ok()
                .and_then(|d| d.file_name().map(|n| n.to_string_lossy().into_owned()))
                .unwrap_or_else(|| "timelapse".into());
            Ok((jobs, p.work_dir(), name, p.settings.fps))
        }
        Err(_) => {
            let from = from.ok_or_else(|| {
                anyhow!("no project here; pass --from with a folder of developed images")
            })?;
            let files = sequence::scan(from)?;
            let centre = MotionKey {
                frame: 0,
                zoom: 1.0,
                cx: 0.5,
                cy: 0.5,
            };
            let jobs = files
                .into_iter()
                .map(|f| FrameJob {
                    input: f,
                    motion: centre,
                })
                .collect();
            let name = from
                .canonicalize()?
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| "timelapse".into());
            Ok((jobs, from.join(".hat_timelapse"), name, 25.0))
        }
    }
}

fn colour_slug(c: OutputSpace) -> &'static str {
    match c {
        OutputSpace::Rec709 => "rec709",
        OutputSpace::DisplayP3 => "p3",
        OutputSpace::Rec2020 => "rec2020",
        OutputSpace::HdrPq => "pq",
        OutputSpace::HdrHlg => "hlg",
        OutputSpace::Linear => "linear",
    }
}

/// Frame into `out` (or a work folder) and return (folder, pattern, fps, name, size).
fn frame(
    dir: &Path,
    a: &FrameArgs,
    out: Option<&Path>,
) -> Result<(PathBuf, String, f64, String, Size)> {
    let size = Size::parse(&a.size)?;
    let colour: OutputSpace = a.colour.into();
    let (jobs, work, name, project_fps) = frame_jobs(dir, a.from.as_deref())?;
    let fps = a.fps.unwrap_or(project_fps);
    let out_dir = out.map(Path::to_path_buf).unwrap_or_else(|| {
        work.join("framed").join(format!(
            "{}x{}-{}",
            size.width,
            size.height,
            colour_slug(colour)
        ))
    });
    let opts = framing::Options {
        size,
        output: colour,
        input: None,
        prefix: "frame".into(),
        overwrite: a.overwrite,
    };
    eprintln!(
        "framing {} images at {}x{} ({})…",
        jobs.len(),
        size.width,
        size.height,
        colour_slug(colour)
    );
    framing::run(&jobs, &out_dir, &opts, fps, |d, t| {
        if d % 5 == 0 || d == t {
            eprint!("\rframing {d}/{t}");
        }
    })?;
    eprintln!();
    Ok((out_dir, "frame_%06d.tif".into(), fps, name, size))
}

pub fn export(dir: &Path, a: &ExportArgs) -> Result<PathBuf> {
    let size = Size::parse(&a.frame.size)?;
    let out = a.out.clone().unwrap_or_else(|| {
        dir.join("export").join(format!(
            "{}x{}-{}",
            size.width,
            size.height,
            colour_slug(a.frame.colour.into())
        ))
    });
    let (out_dir, ..) = frame(dir, &a.frame, Some(&out))?;
    println!(
        "TIFF sequence ready in {} (see sequence.json for frame rate and colour space)",
        out_dir.display()
    );
    Ok(out_dir)
}

pub fn video(dir: &Path, a: &VideoArgs) -> Result<PathBuf> {
    let codec: video::Codec = a.codec.into();
    if !video::ffmpeg_has_encoder(&a.ffmpeg, codec.encoder()) {
        bail!(
            "ffmpeg (`{}`) is missing or was built without the {} encoder",
            a.ffmpeg,
            codec.encoder()
        );
    }
    let colour: OutputSpace = a.frame.colour.into();
    // Fail fast on impossible combinations before spending time framing.
    video::build_command(
        Path::new("."),
        "x",
        Path::new("x.mp4"),
        &video_opts(a, 25.0)?,
    )?;
    let (seq, pattern, fps, name, size) = frame(dir, &a.frame, None)?;
    let out = a.out.clone().unwrap_or_else(|| {
        let tag = format!("{:?}", a.codec.to_possible_value_name()).replace('"', "");
        dir.join(format!(
            "{name}_{}x{}_{}_{tag}.{}",
            size.width,
            size.height,
            colour_slug(colour),
            codec.extension()
        ))
    });
    let mut o = video_opts(a, fps)?;
    o.large_frame = size.width as u64 * size.height as u64 > 4096 * 2160;
    video::encode(&seq, &pattern, &out, &o)?;
    println!("video written to {}", out.display());
    Ok(out)
}

fn video_opts(a: &VideoArgs, fps: f64) -> Result<video::Options> {
    let (cll, fall) = a
        .max_cll
        .split_once(',')
        .and_then(|(x, y)| Some((x.trim().parse().ok()?, y.trim().parse().ok()?)))
        .ok_or_else(|| anyhow!("--max-cll expects two numbers, e.g. 1000,400"))?;
    Ok(video::Options {
        codec: a.codec.into(),
        fps,
        quality: a.quality,
        colour: a.frame.colour.into(),
        motion_blur: a.motion_blur.max(1),
        max_cll: (cll, fall),
        ffmpeg: a.ffmpeg.clone(),
        large_frame: false,
    })
}

trait PossibleValueName {
    fn to_possible_value_name(&self) -> String;
}

impl<T: clap::ValueEnum> PossibleValueName for T {
    fn to_possible_value_name(&self) -> String {
        self.to_possible_value()
            .map(|v| v.get_name().to_string())
            .unwrap_or_default()
    }
}

pub fn parse_weekdays(days: &[String]) -> Result<Vec<u32>> {
    days.iter()
        .flat_map(|d| d.split(','))
        .map(|d| {
            let d = d.trim().to_ascii_lowercase();
            ["mon", "tue", "wed", "thu", "fri", "sat", "sun"]
                .iter()
                .position(|n| d.starts_with(n))
                .map(|i| i as u32)
                .ok_or_else(|| anyhow!("unknown weekday `{d}`"))
        })
        .collect()
}

pub fn parse_period(s: &str) -> Result<f64> {
    let s = s.trim().to_ascii_lowercase();
    let (num, unit) = s.split_at(s.find(|c: char| c.is_ascii_alphabetic()).unwrap_or(s.len()));
    let n: f64 = num.parse().with_context(|| format!("bad period `{s}`"))?;
    let mult = match unit {
        "s" | "" => 1.0,
        "m" | "min" => 60.0,
        "h" => 3600.0,
        "d" => 86400.0,
        "w" => 604800.0,
        _ => bail!("unknown period unit `{unit}` (use s, m, h, d or w)"),
    };
    if n <= 0.0 {
        bail!("period must be positive");
    }
    Ok(n * mult)
}

/// Expand batch arguments into project folders.
fn batch_dirs(dirs: &[PathBuf]) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    for d in dirs {
        let has_images = sequence::scan(d).map(|f| !f.is_empty()).unwrap_or(false);
        if has_images || Project::file_path(d).exists() {
            out.push(d.clone());
            continue;
        }
        let mut subs: Vec<PathBuf> = std::fs::read_dir(d)
            .with_context(|| format!("reading {}", d.display()))?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.is_dir() && sequence::scan(p).map(|f| !f.is_empty()).unwrap_or(false))
            .collect();
        subs.sort_by(|a, b| sequence::natural_cmp(&a.to_string_lossy(), &b.to_string_lossy()));
        if subs.is_empty() {
            eprintln!("warning: no images in {} or its sub-folders", d.display());
        }
        out.extend(subs);
    }
    Ok(out)
}

pub fn batch(a: &BatchArgs) -> Result<()> {
    let steps: Vec<String> = a
        .steps
        .split(',')
        .map(|s| s.trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty())
        .collect();
    const KNOWN: &[&str] = &[
        "init",
        "holy-grail",
        "interpolate",
        "deflicker",
        "render",
        "export",
        "video",
    ];
    if let Some(bad) = steps.iter().find(|s| !KNOWN.contains(&s.as_str())) {
        bail!("unknown batch step `{bad}` (known: {})", KNOWN.join(", "));
    }
    let mut codecs = vec![a.video.codec];
    if let Some(also) = &a.also {
        for c in also.split(',') {
            codecs.push(
                <crate::CodecArg as clap::ValueEnum>::from_str(c.trim(), true)
                    .map_err(|e| anyhow!(e))?,
            );
        }
    }
    let dirs = batch_dirs(&a.dirs)?;
    println!(
        "batch: {} projects, steps {}",
        dirs.len(),
        steps.join(" → ")
    );
    let mut failed = Vec::new();
    for (k, dir) in dirs.iter().enumerate() {
        println!("\n[{}/{}] {}", k + 1, dirs.len(), dir.display());
        let r: Result<()> = (|| {
            for s in &steps {
                match s.as_str() {
                    "init" => {
                        if !Project::file_path(dir).exists() {
                            init(dir, false, 2)?;
                        }
                    }
                    "holy-grail" => holy_grail(dir, 0, 0.05)?,
                    "interpolate" => interpolate(dir, None)?,
                    "deflicker" => deflicker(dir, &a.deflicker)?,
                    "render" => {
                        // HDR and linear outputs need LightCraft's float TIFF.
                        let colour: OutputSpace = a.video.frame.colour.into();
                        let float = colour.is_hdr() || colour == OutputSpace::Linear;
                        render(
                            dir,
                            &RenderArgs {
                                format: "tif".into(),
                                overwrite: false,
                                previews: false,
                                bit_depth: if float { 32 } else { 16 },
                            },
                        )?
                    }
                    "export" => {
                        export(
                            dir,
                            &ExportArgs {
                                frame: a.video.frame.clone(),
                                out: None,
                            },
                        )?;
                    }
                    "video" => {
                        for c in &codecs {
                            let mut v = a.video.clone();
                            v.codec = *c;
                            v.out = None;
                            video(dir, &v)?;
                        }
                    }
                    _ => unreachable!(),
                }
            }
            Ok(())
        })();
        if let Err(e) = r {
            eprintln!("error in {}: {e:#}", dir.display());
            failed.push(dir.clone());
            if !a.keep_going {
                bail!("batch stopped (use --keep-going to continue past failures)");
            }
        }
    }
    println!(
        "\nbatch finished: {} succeeded, {} failed",
        dirs.len() - failed.len(),
        failed.len()
    );
    for f in &failed {
        println!("  failed: {}", f.display());
    }
    if failed.is_empty() {
        Ok(())
    } else {
        bail!("{} projects failed", failed.len())
    }
}

pub fn doctor() -> Result<()> {
    let check = |name: &str, ok: bool, hint: &str| {
        println!(
            "{} {name}{}",
            if ok { "ok     " } else { "missing" },
            if ok {
                String::new()
            } else {
                format!("  ({hint})")
            }
        );
    };
    check(
        "lightcraft-cli",
        lightcraft::check_available("lightcraft-cli"),
        "needed for `hat_timelapse render`; build with `cargo build --release -p lightcraft-cli`",
    );
    let ff = std::process::Command::new("ffmpeg")
        .arg("-version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    check("ffmpeg", ff, "needed for `hat_timelapse video`");
    if ff {
        for (label, enc) in [
            ("  H.264 (libx264)", "libx264"),
            ("  HEVC (libx265)", "libx265"),
            ("  AV1 (libsvtav1)", "libsvtav1"),
            ("  ProRes (prores_ks)", "prores_ks"),
            ("  DNxHR (dnxhd)", "dnxhd"),
        ] {
            check(
                label,
                video::ffmpeg_has_encoder("ffmpeg", enc),
                "use an ffmpeg build that includes it",
            );
        }
    }
    let et = std::process::Command::new("exiftool")
        .arg("-ver")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    check(
        "exiftool",
        et,
        "optional; reads metadata from CR3, RAF and other non-TIFF raws",
    );
    Ok(())
}

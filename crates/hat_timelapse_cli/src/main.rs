//! `hat_timelapse`: the command-line timelapse workflow.

mod workflow;

use anyhow::{bail, Result};
use clap::{Args, Parser, Subcommand, ValueEnum};
use hat_timelapse_core::interp::Method;
use hat_timelapse_render::colour::OutputSpace;
use hat_timelapse_render::video::Codec;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "hat_timelapse", version, about = "Timelapse workflow for LightCraft and Lightroom", long_about = None)]
struct Cli {
    /// Project folder (the folder holding the images).
    #[arg(short, long, global = true, default_value = ".")]
    project: PathBuf,
    /// Worker threads (0 = all cores).
    #[arg(short, long, global = true, default_value_t = 0)]
    jobs: usize,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Scan a folder of images, read capture metadata and create hat_timelapse.json.
    Init {
        /// Replace an existing project.
        #[arg(long)]
        force: bool,
        /// Number of evenly spaced keyframes to mark.
        #[arg(long, default_value_t = 2)]
        keyframes: usize,
    },
    /// Show the state of the project.
    Status,
    /// Choose keyframes.
    #[command(subcommand)]
    Keyframes(KeyCmd),
    /// Compensate camera setting changes (day-to-night "holy grail" sequences).
    HolyGrail {
        /// Frame whose camera settings are the reference.
        #[arg(long, default_value_t = 0)]
        reference: usize,
        /// Ignore EV changes smaller than this.
        #[arg(long, default_value_t = 0.05)]
        threshold: f64,
    },
    /// Read keyframe edits from their sidecars and interpolate across all frames.
    Interpolate {
        #[arg(long, value_enum)]
        method: Option<MethodArg>,
    },
    /// Measure brightness and smooth out flicker.
    Deflicker(DeflickerArgs),
    /// Filters for long-term and construction timelapses.
    #[command(subcommand)]
    Filter(FilterCmd),
    /// Pan and zoom keyframes applied when framing.
    #[command(subcommand)]
    Motion(MotionCmd),
    /// Write develop settings into every frame's XMP sidecar.
    Write,
    /// Develop every included frame with LightCraft into .hat_timelapse/rendered.
    Render(RenderArgs),
    /// Frame and export a numbered TIFF sequence for EffectCraft or FilmCraft.
    Export(ExportArgs),
    /// Frame and encode a video.
    Video(VideoArgs),
    /// Run several steps over many projects.
    Batch(BatchArgs),
    /// Check that LightCraft, ffmpeg and exiftool are available.
    Doctor,
}

#[derive(Subcommand)]
enum KeyCmd {
    /// Mark N evenly spaced keyframes (first and last always included).
    Auto { count: usize },
    /// Mark exactly these frame numbers (0-based).
    Set { frames: Vec<usize> },
    /// List keyframes.
    List,
    /// Label keyframes in their sidecars so they stand out in Lightroom/LightCraft.
    Label,
}

#[derive(Subcommand)]
enum FilterCmd {
    /// Keep frames captured between two hours of the day (may wrap midnight).
    Time { from: f64, to: f64 },
    /// Drop frames from given weekdays, e.g. `sat sun`.
    Weekdays { days: Vec<String> },
    /// Keep one frame per period (e.g. 1d, 6h, 30m), the steadiest in each.
    Every { period: String },
    /// Drop frames far brighter or darker than their neighbours.
    Outliers {
        #[arg(long, default_value_t = 15)]
        window: usize,
        #[arg(long, default_value_t = 1.0)]
        max_ev: f64,
    },
    /// Include every frame again.
    Reset,
}

#[derive(Subcommand)]
enum MotionCmd {
    /// Add a pan/zoom keyframe.
    Add {
        frame: usize,
        #[arg(long, default_value_t = 1.0)]
        zoom: f64,
        #[arg(long, default_value_t = 0.5)]
        cx: f64,
        #[arg(long, default_value_t = 0.5)]
        cy: f64,
    },
    List,
    Clear,
}

#[derive(Clone, Copy, ValueEnum)]
enum MethodArg {
    Linear,
    Smooth,
}

impl From<MethodArg> for Method {
    fn from(m: MethodArg) -> Self {
        match m {
            MethodArg::Linear => Method::Linear,
            MethodArg::Smooth => Method::Smooth,
        }
    }
}

#[derive(Clone, Copy, ValueEnum, PartialEq, Eq)]
pub enum SourceArg {
    /// Source files (raw previews are extracted automatically).
    Originals,
    /// Images in .hat_timelapse/previews rendered with current settings.
    Previews,
}

#[derive(Args, Clone)]
pub struct DeflickerArgs {
    #[arg(long, value_enum, default_value = "originals")]
    pub source: SourceArg,
    /// Smoothing window in frames. Larger = stronger.
    #[arg(long, default_value_t = 15)]
    pub window: usize,
    /// Region of interest x,y,w,h (0..1), e.g. to ignore a changing sky.
    #[arg(long, value_parser = parse_roi)]
    pub roi: Option<[f64; 4]>,
    /// Start from zero rather than refining the existing correction.
    #[arg(long)]
    pub reset: bool,
}

#[derive(Args, Clone)]
pub struct RenderArgs {
    /// Output format for developed frames.
    #[arg(long, default_value = "tif")]
    pub format: String,
    /// Re-render frames that already exist.
    #[arg(long)]
    pub overwrite: bool,
    /// Render small JPEG previews into .hat_timelapse/previews instead (for deflicker).
    #[arg(long)]
    pub previews: bool,
}

#[derive(Args, Clone)]
pub struct FrameArgs {
    /// Folder of developed images. Default: .hat_timelapse/rendered. Use this for a Lightroom export.
    #[arg(long)]
    pub from: Option<PathBuf>,
    /// Output size: hd, fhd, 2.7k, 4k/uhd, dci4k, 5k, 6k, 8k, dci8k, 12k or WIDTHxHEIGHT.
    #[arg(long, default_value = "4k")]
    pub size: String,
    #[arg(long, value_enum, default_value = "rec709")]
    pub colour: ColourArg,
    /// Frames per second (default from project, else 25).
    #[arg(long)]
    pub fps: Option<f64>,
    /// Re-frame images that already exist.
    #[arg(long)]
    pub overwrite: bool,
}

#[derive(Args, Clone)]
pub struct ExportArgs {
    #[command(flatten)]
    pub frame: FrameArgs,
    /// Output folder. Default: <project>/export/<size>-<colour>.
    #[arg(long)]
    pub out: Option<PathBuf>,
}

#[derive(Args, Clone)]
pub struct VideoArgs {
    #[command(flatten)]
    pub frame: FrameArgs,
    #[arg(long, value_enum, default_value = "h264")]
    pub codec: CodecArg,
    /// CRF for H.264/HEVC/AV1 (lower is better quality).
    #[arg(long, default_value_t = 16)]
    pub quality: u32,
    /// Blend N frames for motion blur.
    #[arg(long, default_value_t = 1)]
    pub motion_blur: u32,
    /// HDR10 MaxCLL,MaxFALL in nits.
    #[arg(long, default_value = "1000,400")]
    pub max_cll: String,
    /// Output file. Default: <project>/<name>_<size>_<codec>.<ext>.
    #[arg(long)]
    pub out: Option<PathBuf>,
    #[arg(long, default_value = "ffmpeg")]
    pub ffmpeg: String,
}

#[derive(Args, Clone)]
pub struct BatchArgs {
    /// Project folders, or parent folders whose sub-folders are projects.
    #[arg(required = true)]
    pub dirs: Vec<PathBuf>,
    /// Comma-separated steps: init, holy-grail, interpolate, deflicker, render, export, video.
    #[arg(
        long,
        default_value = "init,holy-grail,interpolate,deflicker,render,video"
    )]
    pub steps: String,
    /// Carry on with the next project if one fails.
    #[arg(long)]
    pub keep_going: bool,
    #[command(flatten)]
    pub deflicker: DeflickerArgs,
    #[command(flatten)]
    pub video: VideoArgs,
    /// Extra video codecs to render in the same pass, e.g. `prores422-hq,hevc`.
    #[arg(long)]
    pub also: Option<String>,
}

#[derive(Clone, Copy, ValueEnum, PartialEq, Eq)]
pub enum ColourArg {
    Rec709,
    DisplayP3,
    Rec2020,
    HdrPq,
    HdrHlg,
    Linear,
}

impl From<ColourArg> for OutputSpace {
    fn from(c: ColourArg) -> Self {
        match c {
            ColourArg::Rec709 => OutputSpace::Rec709,
            ColourArg::DisplayP3 => OutputSpace::DisplayP3,
            ColourArg::Rec2020 => OutputSpace::Rec2020,
            ColourArg::HdrPq => OutputSpace::HdrPq,
            ColourArg::HdrHlg => OutputSpace::HdrHlg,
            ColourArg::Linear => OutputSpace::Linear,
        }
    }
}

#[derive(Clone, Copy, ValueEnum, PartialEq, Eq)]
pub enum CodecArg {
    H264,
    Hevc,
    Av1,
    Prores422Hq,
    Prores4444,
    Prores4444Xq,
    DnxhrHq,
    DnxhrHqx,
    Dnxhr444,
}

impl From<CodecArg> for Codec {
    fn from(c: CodecArg) -> Self {
        match c {
            CodecArg::H264 => Codec::H264,
            CodecArg::Hevc => Codec::Hevc,
            CodecArg::Av1 => Codec::Av1,
            CodecArg::Prores422Hq => Codec::Prores422Hq,
            CodecArg::Prores4444 => Codec::Prores4444,
            CodecArg::Prores4444Xq => Codec::Prores4444Xq,
            CodecArg::DnxhrHq => Codec::DnxhrHq,
            CodecArg::DnxhrHqx => Codec::DnxhrHqx,
            CodecArg::Dnxhr444 => Codec::Dnxhr444,
        }
    }
}

fn parse_roi(s: &str) -> Result<[f64; 4], String> {
    let v: Vec<f64> = s
        .split(',')
        .map(|x| x.trim().parse::<f64>().map_err(|e| e.to_string()))
        .collect::<Result<_, _>>()?;
    match v.as_slice() {
        [x, y, w, h] if *w > 0.0 && *h > 0.0 => Ok([*x, *y, *w, *h]),
        _ => Err("expected x,y,w,h with positive width and height".into()),
    }
}

fn main() {
    let cli = Cli::parse();
    if let Err(e) = run(cli) {
        eprintln!("error: {e:#}");
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> Result<()> {
    let jobs = cli.jobs;
    hat_timelapse_render::with_jobs(jobs, move || dispatch(cli))?
}

fn dispatch(cli: Cli) -> Result<()> {
    use workflow as w;
    let dir = cli.project;
    match cli.cmd {
        Cmd::Init { force, keyframes } => w::init(&dir, force, keyframes).map(|_| ()),
        Cmd::Status => w::status(&dir),
        Cmd::Keyframes(k) => {
            let mut p = w::load(&dir)?;
            match k {
                KeyCmd::Auto { count } => p.auto_keyframes(count),
                KeyCmd::Set { frames } => {
                    if let Some(&bad) = frames.iter().find(|&&i| i >= p.frames.len()) {
                        bail!("frame {bad} is out of range (0..{})", p.frames.len());
                    }
                    for (i, f) in p.frames.iter_mut().enumerate() {
                        f.keyframe = frames.contains(&i);
                    }
                }
                KeyCmd::List => {}
                KeyCmd::Label => w::label_keyframes(&p)?,
            }
            p.save()?;
            for i in p.keyframe_indices() {
                println!("{i:6}  {}", p.frames[i].file);
            }
            Ok(())
        }
        Cmd::HolyGrail {
            reference,
            threshold,
        } => w::holy_grail(&dir, reference, threshold),
        Cmd::Interpolate { method } => w::interpolate(&dir, method.map(Into::into)),
        Cmd::Deflicker(a) => w::deflicker(&dir, &a),
        Cmd::Filter(f) => {
            use hat_timelapse_core::longterm as lt;
            let mut p = w::load(&dir)?;
            let n = match f {
                FilterCmd::Time { from, to } => lt::time_of_day(&mut p, from, to),
                FilterCmd::Weekdays { days } => lt::weekdays(&mut p, &w::parse_weekdays(&days)?),
                FilterCmd::Every { period } => {
                    lt::one_per_period(&mut p, w::parse_period(&period)?)
                }
                FilterCmd::Outliers { window, max_ev } => {
                    w::ensure_luminance(&mut p, None)?;
                    lt::brightness_outliers(&mut p, window, max_ev)
                }
                FilterCmd::Reset => {
                    lt::reset(&mut p);
                    0
                }
            };
            p.save()?;
            println!(
                "excluded {n} frames; {} of {} remain",
                p.included_indices().len(),
                p.frames.len()
            );
            Ok(())
        }
        Cmd::Motion(m) => {
            let mut p = w::load(&dir)?;
            match m {
                MotionCmd::Add {
                    frame,
                    zoom,
                    cx,
                    cy,
                } => {
                    if zoom < 1.0 {
                        bail!("zoom must be 1.0 or more");
                    }
                    p.motion.retain(|k| k.frame != frame);
                    p.motion.push(hat_timelapse_core::MotionKey {
                        frame,
                        zoom,
                        cx,
                        cy,
                    });
                    p.motion.sort_by_key(|k| k.frame);
                }
                MotionCmd::Clear => p.motion.clear(),
                MotionCmd::List => {}
            }
            p.save()?;
            for k in &p.motion {
                println!(
                    "frame {:6}  zoom {:.3}  centre {:.3},{:.3}",
                    k.frame, k.zoom, k.cx, k.cy
                );
            }
            Ok(())
        }
        Cmd::Write => {
            let mut p = w::load(&dir)?;
            w::write_sidecars(&mut p)?;
            p.save()
        }
        Cmd::Render(a) => w::render(&dir, &a),
        Cmd::Export(a) => w::export(&dir, &a).map(|_| ()),
        Cmd::Video(a) => w::video(&dir, &a).map(|_| ()),
        Cmd::Batch(a) => w::batch(&a),
        Cmd::Doctor => w::doctor(),
    }
}

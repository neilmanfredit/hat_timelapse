//! The main window.

use crate::preview::Preview;
use crate::runner::Job;
use crate::theme::{self, GOLD, GOLD_SOFT, NAVY_LINE, SKY, TEXT_DIM};
use egui::{Align, Color32, Layout, RichText};
use egui_extras::{Column, TableBuilder};
use hat_timelapse_core::params::EXPOSURE;
use hat_timelapse_core::project::{Project, WORK_DIR};
use hat_timelapse_core::sequence;
use hat_timelapse_render::framing::{self, Size};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------- options

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
struct VideoOpts {
    size: String,
    custom: [u32; 2],
    codec: String,
    colour: String,
    fps: f64,
    quality: u32,
    motion_blur: u32,
    #[serde(skip)]
    from: Option<PathBuf>,
    #[serde(skip)]
    out: Option<PathBuf>,
}

impl Default for VideoOpts {
    fn default() -> Self {
        VideoOpts {
            size: "4k".into(),
            custom: [3840, 2160],
            codec: "h264".into(),
            colour: "rec709".into(),
            fps: 25.0,
            quality: 16,
            motion_blur: 1,
            from: None,
            out: None,
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
struct ExportOpts {
    size: String,
    custom: [u32; 2],
    colour: String,
    #[serde(skip)]
    from: Option<PathBuf>,
    #[serde(skip)]
    out: Option<PathBuf>,
}

impl Default for ExportOpts {
    fn default() -> Self {
        ExportOpts {
            size: "4k".into(),
            custom: [3840, 2160],
            colour: "rec709".into(),
            from: None,
            out: None,
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
struct DeflickerOpts {
    window: usize,
    use_roi: bool,
    roi: [f64; 4],
    reset: bool,
    passes: usize,
}

impl Default for DeflickerOpts {
    fn default() -> Self {
        DeflickerOpts {
            window: 15,
            use_roi: false,
            roi: [0.0, 0.5, 1.0, 0.5],
            reset: false,
            passes: 2,
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
struct FilterOpts {
    from: f64,
    to: f64,
    days: [bool; 7],
    period: String,
    outlier_window: usize,
    max_ev: f64,
}

impl Default for FilterOpts {
    fn default() -> Self {
        FilterOpts {
            from: 7.0,
            to: 18.0,
            days: [false, false, false, false, false, true, true],
            period: "1d".into(),
            outlier_window: 15,
            max_ev: 1.0,
        }
    }
}

const BATCH_STEPS: [&str; 7] = [
    "init",
    "holy-grail",
    "interpolate",
    "deflicker",
    "render",
    "export",
    "video",
];

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
struct BatchOpts {
    steps: [bool; 7],
    also: String,
    keep_going: bool,
    #[serde(skip)]
    dirs: Vec<PathBuf>,
}

impl Default for BatchOpts {
    fn default() -> Self {
        BatchOpts {
            steps: [true, true, true, true, true, false, true],
            also: String::new(),
            keep_going: true,
            dirs: Vec::new(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
struct Prefs {
    recent: Vec<PathBuf>,
    keyframes: usize,
    holy_threshold: f64,
    linear: bool,
    bit_depth: u8,
    overwrite: bool,
    video: VideoOpts,
    export: ExportOpts,
    deflicker: DeflickerOpts,
    filters: FilterOpts,
    batch: BatchOpts,
    show_developed: bool,
}

impl Default for Prefs {
    fn default() -> Self {
        Prefs {
            recent: Vec::new(),
            keyframes: 3,
            holy_threshold: 0.05,
            linear: false,
            bit_depth: 16,
            overwrite: false,
            video: VideoOpts::default(),
            export: ExportOpts::default(),
            deflicker: DeflickerOpts::default(),
            filters: FilterOpts::default(),
            batch: BatchOpts::default(),
            show_developed: false,
        }
    }
}

const SIZES: &[(&str, &str)] = &[
    ("hd", "HD 1280×720"),
    ("fhd", "Full HD 1920×1080"),
    ("2.7k", "2.7K 2704×1520"),
    ("4k", "4K UHD 3840×2160"),
    ("dci4k", "4K DCI 4096×2160"),
    ("5k", "5K 5120×2880"),
    ("6k", "6K 6144×3456"),
    ("8k", "8K UHD 7680×4320"),
    ("dci8k", "8K DCI 8192×4320"),
    ("12k", "12K 12288×6480"),
    ("custom", "Custom…"),
];

const CODECS: &[(&str, &str)] = &[
    ("h264", "H.264 (MP4) – plays everywhere"),
    ("hevc", "H.265 / HEVC (MP4) – smaller, HDR"),
    ("av1", "AV1 (MP4) – smallest, HDR"),
    ("prores422-hq", "ProRes 422 HQ (MOV)"),
    ("prores4444", "ProRes 4444 (MOV)"),
    ("prores4444-xq", "ProRes 4444 XQ (MOV)"),
    ("dnxhr-hq", "DNxHR HQ (MOV)"),
    ("dnxhr-hqx", "DNxHR HQX 10-bit (MOV)"),
    ("dnxhr444", "DNxHR 444 (MOV)"),
];

const COLOURS: &[(&str, &str)] = &[
    ("rec709", "Rec.709 – standard video"),
    ("display-p3", "Display P3 – Apple wide colour"),
    ("rec2020", "Rec.2020 – wide-colour SDR"),
    ("hdr-pq", "HDR10 (PQ)"),
    ("hdr-hlg", "HDR HLG"),
    ("linear", "Linear float (TIFF only)"),
];

const WEEKDAYS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];

// ---------------------------------------------------------------- app

struct Step {
    label: String,
    dir: Option<PathBuf>,
    args: Vec<String>,
}

#[derive(Default)]
struct Windows {
    video: bool,
    export: bool,
    deflicker: bool,
    filters: bool,
    motion: bool,
    batch: bool,
    help: bool,
}

pub struct HatApp {
    prefs: Prefs,
    dir: Option<PathBuf>,
    project: Option<Project>,
    images_found: usize,
    load_error: Option<String>,
    selected: usize,
    scroll_to_selected: bool,
    job: Option<Job>,
    queue: VecDeque<Step>,
    log: Vec<String>,
    show_log: bool,
    preview: Preview,
    win: Windows,
    motion: [f64; 3],
}

impl HatApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let prefs: Prefs = cc
            .storage
            .and_then(|s| eframe::get_value(s, eframe::APP_KEY))
            .unwrap_or_default();
        let mut app = HatApp {
            prefs,
            dir: None,
            project: None,
            images_found: 0,
            load_error: None,
            selected: 0,
            scroll_to_selected: false,
            job: None,
            queue: VecDeque::new(),
            log: Vec::new(),
            show_log: false,
            preview: Preview::new(&cc.egui_ctx),
            win: Windows::default(),
            motion: [1.0, 0.5, 0.5],
        };
        // A folder given on the command line (or dropped on the app icon) opens straight away.
        if let Some(arg) = std::env::args_os()
            .nth(1)
            .map(PathBuf::from)
            .filter(|p| p.is_dir())
        {
            app.open(arg);
        }
        app
    }

    fn open(&mut self, dir: PathBuf) {
        self.prefs.recent.retain(|d| d != &dir);
        self.prefs.recent.insert(0, dir.clone());
        self.prefs.recent.truncate(10);
        self.dir = Some(dir);
        self.selected = 0;
        self.preview.clear();
        self.reload();
    }

    fn reload(&mut self) {
        let Some(dir) = &self.dir else { return };
        self.load_error = None;
        if Project::file_path(dir).exists() {
            match Project::load(dir) {
                Ok(p) => {
                    self.selected = self.selected.min(p.frames.len().saturating_sub(1));
                    self.project = Some(p);
                }
                Err(e) => {
                    self.project = None;
                    self.load_error = Some(format!("{e:#}"));
                }
            }
        } else {
            self.project = None;
            self.images_found = sequence::scan(dir).map(|f| f.len()).unwrap_or(0);
        }
    }

    fn busy(&self) -> bool {
        self.job.is_some() || !self.queue.is_empty()
    }

    fn run(&mut self, label: &str, args: &[&str]) {
        let dir = self.dir.clone();
        self.queue.push_back(Step {
            label: label.to_string(),
            dir,
            args: args.iter().map(|s| s.to_string()).collect(),
        });
    }

    fn run_owned(&mut self, label: &str, dir: Option<PathBuf>, args: Vec<String>) {
        self.queue.push_back(Step {
            label: label.to_string(),
            dir,
            args,
        });
    }

    fn pump_jobs(&mut self, ctx: &egui::Context) {
        if let Some(job) = &mut self.job {
            self.log.extend(job.poll());
            if let Some(ok) = job.finished {
                let label = job.label.clone();
                self.job = None;
                if ok {
                    self.log.push(format!("✔ {label} finished"));
                } else {
                    self.log
                        .push(format!("✖ {label} did not finish; see the messages above"));
                    if !self.queue.is_empty() {
                        self.log.push(format!(
                            "  {} queued steps were cancelled",
                            self.queue.len()
                        ));
                    }
                    self.queue.clear();
                    self.show_log = true;
                }
                self.reload();
                self.preview.clear();
            }
        }
        if self.job.is_none() {
            if let Some(step) = self.queue.pop_front() {
                self.job = Some(Job::start(
                    &step.label,
                    step.dir.as_deref(),
                    &step.args,
                    ctx.clone(),
                ));
            }
        }
        if self.log.len() > 4000 {
            self.log.drain(0..1000);
        }
    }

    fn save_project(&mut self) {
        if let Some(p) = &self.project {
            if let Err(e) = p.save() {
                self.log.push(format!("Could not save the project: {e:#}"));
            }
        }
    }

    fn pick_folder(&mut self) {
        if let Some(d) = rfd::FileDialog::new()
            .set_title("Choose a timelapse sequence folder")
            .pick_folder()
        {
            self.open(d);
        }
    }
}

impl eframe::App for HatApp {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, &self.prefs);
    }

    /// Quitting stops any step that is still running, rather than leaving it
    /// working in the background.
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.queue.clear();
        if let Some(job) = &self.job {
            job.cancel();
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.pump_jobs(&ctx);
        if self.busy() {
            ctx.request_repaint_after(std::time::Duration::from_millis(200));
        }
        self.keyboard(&ctx);

        egui::Panel::top("top")
            .frame(bar_frame())
            .show(ui, |ui| self.top_bar(ui));
        egui::Panel::bottom("status")
            .frame(bar_frame())
            .show(ui, |ui| self.status_bar(ui));
        if self.show_log {
            egui::Panel::bottom("log")
                .resizable(true)
                .default_size(180.0)
                .show(ui, |ui| self.log_panel(ui));
        }
        if self.project.is_some() {
            egui::Panel::bottom("graph")
                .resizable(true)
                .default_size(230.0)
                .min_size(140.0)
                .show(ui, |ui| self.graph(ui));
            egui::Panel::right("preview")
                .resizable(true)
                .default_size(460.0)
                .min_size(280.0)
                .show(ui, |ui| self.preview_panel(ui));
        }
        egui::CentralPanel::default().show(ui, |ui| {
            if self.project.is_some() {
                self.table(ui);
            } else {
                self.welcome(ui);
            }
        });

        self.windows(&ctx);
    }
}

/// Open windows in the middle of the app rather than at its top-left corner.
fn centred<'a>(w: egui::Window<'a>, ctx: &egui::Context) -> egui::Window<'a> {
    w.pivot(egui::Align2::CENTER_CENTER)
        .default_pos(ctx.content_rect().center())
}

fn bar_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(theme::NAVY_DEEP)
        .inner_margin(egui::Margin::symmetric(12, 8))
}

// ---------------------------------------------------------------- panels

impl HatApp {
    fn keyboard(&mut self, ctx: &egui::Context) {
        if ctx.memory(|m| m.focused().is_some()) {
            return;
        }
        let Some(n) = self.project.as_ref().map(|p| p.frames.len()) else {
            return;
        };
        if n == 0 {
            return;
        }
        let (up, down, home, end, key, excl) = ctx.input(|i| {
            (
                i.key_pressed(egui::Key::ArrowUp) || i.key_pressed(egui::Key::ArrowLeft),
                i.key_pressed(egui::Key::ArrowDown) || i.key_pressed(egui::Key::ArrowRight),
                i.key_pressed(egui::Key::Home),
                i.key_pressed(egui::Key::End),
                i.key_pressed(egui::Key::K),
                i.key_pressed(egui::Key::X),
            )
        });
        let before = self.selected;
        if up {
            self.selected = self.selected.saturating_sub(1);
        }
        if down {
            self.selected = (self.selected + 1).min(n - 1);
        }
        if home {
            self.selected = 0;
        }
        if end {
            self.selected = n - 1;
        }
        if before != self.selected {
            self.scroll_to_selected = true;
        }
        if (key || excl) && !self.busy() {
            let i = self.selected;
            if let Some(p) = &mut self.project {
                if key {
                    p.frames[i].keyframe = !p.frames[i].keyframe;
                } else {
                    p.frames[i].excluded = !p.frames[i].excluded;
                }
            }
            self.save_project();
        }
    }

    fn top_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("🎩 Hat Timelapse")
                    .size(18.0)
                    .strong()
                    .color(GOLD),
            );
            ui.add_space(12.0);
            if ui
                .button("📂 Open…")
                .on_hover_text("Choose a folder holding one timelapse sequence")
                .clicked()
            {
                self.pick_folder();
            }
            let recent = self.prefs.recent.clone();
            ui.menu_button("Recent", |ui| {
                if recent.is_empty() {
                    ui.label("No recent folders");
                }
                for d in recent {
                    if ui.button(d.display().to_string()).clicked() {
                        self.open(d);
                        ui.close();
                    }
                }
            });
            if let Some(d) = &self.dir {
                ui.label(
                    RichText::new(d.display().to_string())
                        .color(TEXT_DIM)
                        .small(),
                );
            }

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.menu_button("Tools", |ui| {
                    let has = self.project.is_some();
                    if ui
                        .add_enabled(has, egui::Button::new("Long-term filters…"))
                        .clicked()
                    {
                        self.win.filters = true;
                        ui.close();
                    }
                    if ui
                        .add_enabled(has, egui::Button::new("Pan & zoom…"))
                        .clicked()
                    {
                        self.win.motion = true;
                        ui.close();
                    }
                    if ui.button("Batch…").clicked() {
                        self.win.batch = true;
                        ui.close();
                    }
                    ui.separator();
                    if ui
                        .button("Check tools (LightCraft, ffmpeg, exiftool)")
                        .clicked()
                    {
                        self.run_owned("Check tools", None, vec!["doctor".into()]);
                        self.show_log = true;
                        ui.close();
                    }
                    if ui
                        .add_enabled(self.dir.is_some(), egui::Button::new("Show folder"))
                        .clicked()
                    {
                        if let Some(d) = &self.dir {
                            reveal(d);
                        }
                        ui.close();
                    }
                    if ui
                        .add_enabled(self.dir.is_some(), egui::Button::new("Reload project"))
                        .clicked()
                    {
                        self.reload();
                        self.preview.clear();
                        ui.close();
                    }
                    ui.separator();
                    if ui.button("How it works").clicked() {
                        self.win.help = true;
                        ui.close();
                    }
                });
            });
        });

        if self.project.is_some() {
            ui.add_space(4.0);
            ui.horizontal_wrapped(|ui| self.steps(ui));
        }
    }

    fn steps(&mut self, ui: &mut egui::Ui) {
        let busy = self.busy();
        let st = self.project.as_ref().map(StepState::of).unwrap_or_default();
        let n = self.project.as_ref().map(|p| p.frames.len()).unwrap_or(0);

        ui.add_enabled_ui(!busy, |ui| {
            // 1 Keyframes
            step_menu(ui, 1, "Keyframes", st.keyframes > 0, &format!("{}", st.keyframes), |ui| {
                theme::eyebrow(ui, "Keyframes");
                ui.label("Frames you edit by hand. Use more where the light changes quickly.");
                ui.horizontal(|ui| {
                    ui.label("Evenly spaced:");
                    ui.add(egui::DragValue::new(&mut self.prefs.keyframes).range(1..=n.max(1)));
                    if ui.button("Set").clicked() {
                        let c = self.prefs.keyframes.to_string();
                        self.run("Set keyframes", &["keyframes", "auto", &c]);
                        ui.close();
                    }
                });
                if ui.button(format!("Toggle frame {} as keyframe   (K)", self.selected)).clicked() {
                    let i = self.selected;
                    if let Some(p) = &mut self.project {
                        p.frames[i].keyframe = !p.frames[i].keyframe;
                    }
                    self.save_project();
                    ui.close();
                }
                ui.separator();
                if ui.add(theme::primary("Label keyframes for my editor")).clicked() {
                    self.run("Label keyframes", &["keyframes", "label"]);
                    ui.close();
                }
                ui.label(
                    RichText::new(
                        "Then edit the keyframes in LightCraft or Lightroom and save metadata to file (⌘S). \
                         Set the overall look on the first keyframe.",
                    )
                    .color(TEXT_DIM)
                    .small(),
                );
            });

            // 2 Holy grail
            step_menu(ui, 2, "Holy grail", st.holy_grail, "", |ui| {
                theme::eyebrow(ui, "Holy grail");
                ui.label("Cancels the brightness jumps from shutter, aperture or ISO changes during the shoot.");
                ui.label(RichText::new("Skip this if you did not change exposure while shooting.").color(TEXT_DIM).small());
                ui.horizontal(|ui| {
                    ui.label("Ignore changes under");
                    ui.add(egui::DragValue::new(&mut self.prefs.holy_threshold).range(0.0..=1.0).speed(0.01).suffix(" EV"));
                });
                if ui.add(theme::primary("Run holy grail")).clicked() {
                    let t = format!("{}", self.prefs.holy_threshold);
                    self.run("Holy grail", &["holy-grail", "--threshold", &t]);
                    ui.close();
                }
            });

            // 3 Read keyframes
            step_menu(ui, 3, "Read keyframes", st.interpolated, "", |ui| {
                theme::eyebrow(ui, "Read keyframes");
                ui.label("Reads your keyframe edits and blends them across every frame.");
                ui.horizontal(|ui| {
                    ui.radio_value(&mut self.prefs.linear, false, "Smooth");
                    ui.radio_value(&mut self.prefs.linear, true, "Linear");
                });
                if ui.add(theme::primary("Read and blend keyframes")).clicked() {
                    let m = if self.prefs.linear { "linear" } else { "smooth" };
                    self.run("Read keyframes", &["interpolate", "--method", m]);
                    ui.close();
                }
                ui.label(
                    RichText::new("Afterwards, use Read Metadata from File in your editor to see the result there.")
                        .color(TEXT_DIM)
                        .small(),
                );
            });

            // 4 Deflicker
            if step_button(ui, 4, "Deflicker", st.deflickered).clicked() {
                self.win.deflicker = true;
            }

            // 5 Render
            step_menu(ui, 5, "Render", st.rendered > 0, &format!("{}/{}", st.rendered, st.included), |ui| {
                theme::eyebrow(ui, "Render with LightCraft");
                ui.label("Develops every included frame at full resolution.");
                ui.horizontal(|ui| {
                    ui.radio_value(&mut self.prefs.bit_depth, 16, "16-bit");
                    ui.radio_value(&mut self.prefs.bit_depth, 32, "32-bit float (HDR)");
                });
                ui.checkbox(&mut self.prefs.overwrite, "Re-render frames already done");
                if ui.add(theme::primary("Render")).clicked() {
                    let bd = self.prefs.bit_depth.to_string();
                    let mut a = vec!["render", "--bit-depth", &bd];
                    if self.prefs.overwrite {
                        a.push("--overwrite");
                    }
                    self.run("Render", &a);
                    ui.close();
                }
                if ui.button("Render small previews only").clicked() {
                    self.run("Render previews", &["render", "--previews"]);
                    ui.close();
                }
                ui.label(
                    RichText::new("Developing in Lightroom instead? Export there and use Video → Developed frames from.")
                        .color(TEXT_DIM)
                        .small(),
                );
            });

            // 6 Video, 7 TIFF
            if step_button(ui, 6, "Video", false).clicked() {
                self.win.video = true;
            }
            if step_button(ui, 7, "TIFF export", false).clicked() {
                self.win.export = true;
            }
        });
    }

    fn welcome(&mut self, ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            ui.add_space(ui.available_height() * 0.12);
            if let Some(dir) = self.dir.clone() {
                theme::eyebrow(ui, "New sequence");
                ui.heading(RichText::new(folder_name(&dir)).size(28.0));
                ui.add_space(8.0);
                if let Some(e) = &self.load_error {
                    ui.colored_label(theme::WARN, format!("The project file could not be read: {e}"));
                } else if self.images_found == 0 {
                    ui.label("No images found in this folder. Choose the folder that holds the frames themselves.");
                } else {
                    ui.label(format!("{} images found.", self.images_found));
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        ui.add_space((ui.available_width() - 340.0).max(0.0) / 2.0);
                        ui.label("Keyframes to start with:");
                        ui.add(egui::DragValue::new(&mut self.prefs.keyframes).range(1..=self.images_found.max(1)));
                    });
                    ui.add_space(10.0);
                    let busy = self.busy();
                    if ui.add_enabled(!busy, theme::primary("Create project").min_size(egui::vec2(220.0, 36.0))).clicked() {
                        let k = self.prefs.keyframes.to_string();
                        self.run("Create project", &["init", "--keyframes", &k]);
                    }
                    if busy {
                        ui.spinner();
                    }
                }
                ui.add_space(16.0);
                if ui.button("Choose another folder…").clicked() {
                    self.pick_folder();
                }
            } else {
                theme::eyebrow(ui, "Timelapse workflow");
                ui.heading(RichText::new("Hat Timelapse").size(34.0).color(GOLD));
                ui.add_space(6.0);
                ui.label("Edit a few keyframes. Hat Timelapse does the rest: holy grail, deflicker, rendering and video.");
                ui.add_space(18.0);
                if ui.add(theme::primary("Open a sequence folder…").min_size(egui::vec2(260.0, 40.0))).clicked() {
                    self.pick_folder();
                }
                ui.add_space(20.0);
                if !self.prefs.recent.is_empty() {
                    theme::eyebrow(ui, "Recent");
                    for d in self.prefs.recent.clone() {
                        if ui.link(d.display().to_string()).clicked() {
                            self.open(d);
                        }
                    }
                }
            }
        });
    }

    fn table(&mut self, ui: &mut egui::Ui) {
        let Some(p) = &self.project else { return };
        let n = p.frames.len();
        let mut select: Option<usize> = None;
        let mut toggle_key: Option<usize> = None;
        let mut toggle_use: Option<usize> = None;
        let busy = self.busy();
        let mut tb = TableBuilder::new(ui)
            .striped(true)
            .resizable(true)
            .sense(egui::Sense::click())
            .cell_layout(Layout::left_to_right(Align::Center))
            .column(Column::exact(48.0))
            .column(Column::initial(150.0).at_least(80.0).clip(true))
            .column(Column::exact(34.0))
            .column(Column::exact(34.0))
            .column(Column::initial(72.0))
            .column(Column::initial(64.0))
            .column(Column::initial(48.0))
            .column(Column::initial(52.0))
            .column(Column::initial(76.0))
            .column(Column::initial(76.0))
            .column(Column::initial(76.0))
            .column(Column::remainder().at_least(70.0));
        if self.scroll_to_selected {
            tb = tb.scroll_to_row(self.selected, Some(Align::Center));
            self.scroll_to_selected = false;
        }
        let selected = self.selected;
        tb.header(24.0, |mut h| {
            for (t, tip) in [
                ("#", "Frame number"),
                ("File", ""),
                ("Key", "Keyframe (K)"),
                ("Use", "Included in the video (X)"),
                ("Time", "Capture time"),
                ("Shutter", ""),
                ("f/", "Aperture"),
                ("ISO", ""),
                ("Holy grail", "Exposure offset cancelling camera changes"),
                ("Deflicker", "Exposure correction from deflicker"),
                ("Exposure", "Final exposure written for this frame"),
                ("Brightness", "Measured brightness (stops)"),
            ] {
                h.col(|ui| {
                    let r = ui.label(RichText::new(t).color(GOLD_SOFT).strong());
                    if !tip.is_empty() {
                        r.on_hover_text(tip);
                    }
                });
            }
        })
        .body(|body| {
            body.rows(22.0, n, |mut row| {
                let i = row.index();
                let f = &p.frames[i];
                row.set_selected(i == selected);
                let dim = f.excluded;
                let txt = |s: String| {
                    if dim {
                        RichText::new(s).color(TEXT_DIM)
                    } else {
                        RichText::new(s)
                    }
                };
                row.col(|ui| {
                    ui.label(txt(i.to_string()));
                });
                row.col(|ui| {
                    ui.label(txt(f.file.clone()));
                });
                row.col(|ui| {
                    let t = if f.keyframe {
                        RichText::new("◆").color(GOLD).size(15.0)
                    } else {
                        RichText::new("◇").color(NAVY_LINE)
                    };
                    if ui
                        .add_enabled(!busy, egui::Button::new(t).frame(false))
                        .clicked()
                    {
                        toggle_key = Some(i);
                    }
                });
                row.col(|ui| {
                    let t = if f.excluded {
                        RichText::new("✖").color(theme::WARN)
                    } else {
                        RichText::new("✔").color(theme::GOOD)
                    };
                    if ui
                        .add_enabled(!busy, egui::Button::new(t).frame(false))
                        .clicked()
                    {
                        toggle_use = Some(i);
                    }
                });
                row.col(|ui| {
                    let t = f
                        .exif
                        .captured
                        .as_deref()
                        .and_then(|c| c.split(' ').nth(1))
                        .unwrap_or("")
                        .to_string();
                    ui.label(txt(t.split('.').next().unwrap_or("").to_string()));
                });
                row.col(|ui| {
                    ui.label(txt(f.exif.shutter_s.map(shutter).unwrap_or_default()));
                });
                row.col(|ui| {
                    ui.label(txt(f
                        .exif
                        .aperture
                        .map(|a| format!("{a:.1}"))
                        .unwrap_or_default()));
                });
                row.col(|ui| {
                    ui.label(txt(f
                        .exif
                        .iso
                        .map(|v| format!("{v:.0}"))
                        .unwrap_or_default()));
                });
                row.col(|ui| {
                    ui.label(txt(ev(f.holy_grail_ev)));
                });
                row.col(|ui| {
                    ui.label(txt(ev(f.deflicker_ev)));
                });
                row.col(|ui| {
                    ui.label(txt(ev(p.final_value(i, EXPOSURE).unwrap_or(0.0))));
                });
                row.col(|ui| {
                    ui.label(txt(f
                        .luminance
                        .map(|l| format!("{l:.2}"))
                        .unwrap_or_default()));
                });
                if row.response().clicked() {
                    select = Some(i);
                }
            });
        });
        if let Some(i) = select {
            self.selected = i;
        }
        if toggle_key.is_some() || toggle_use.is_some() {
            if let Some(p) = &mut self.project {
                if let Some(i) = toggle_key {
                    p.frames[i].keyframe = !p.frames[i].keyframe;
                }
                if let Some(i) = toggle_use {
                    p.frames[i].excluded = !p.frames[i].excluded;
                }
            }
            self.save_project();
        }
    }

    fn preview_panel(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let Some(p) = &self.project else { return };
        if p.frames.is_empty() {
            return;
        }
        let i = self.selected.min(p.frames.len() - 1);
        let f = p.frames[i].clone();
        let original = p.frame_path(i);
        let stem = Path::new(&f.file)
            .file_stem()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        let developed = p
            .dir
            .join(WORK_DIR)
            .join("previews")
            .join(format!("{stem}.jpg"));
        let has_developed = developed.exists();
        let track = p.motion_track();
        let motion = track.get(i).copied();

        ui.horizontal(|ui| {
            theme::eyebrow(ui, &format!("Frame {i}"));
            if f.keyframe {
                ui.label(RichText::new("◆ KEYFRAME").color(GOLD).small().strong());
            }
            if f.excluded {
                ui.label(
                    RichText::new("LEFT OUT")
                        .color(theme::WARN)
                        .small()
                        .strong(),
                );
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.add_enabled_ui(has_developed, |ui| {
                    ui.checkbox(&mut self.prefs.show_developed, "Developed")
                        .on_hover_text(
                            "Show the developed preview (Render → Render small previews only)",
                        );
                });
            });
        });
        ui.label(RichText::new(&f.file).small().color(TEXT_DIM));

        let path = if self.prefs.show_developed && has_developed {
            developed
        } else {
            original
        };
        let avail = ui.available_size();
        let max = egui::vec2(avail.x, (avail.y - 150.0).max(160.0));
        match self.preview.get(&ctx, &path) {
            Ok(Some(tex)) => {
                let ts = tex.size_vec2();
                let scale = (max.x / ts.x).min(max.y / ts.y).min(1.5);
                let resp = ui.add(egui::Image::new((tex.id(), ts * scale)));
                // Video framing (16:9 for the chosen size) with pan/zoom.
                if let Some(m) = motion {
                    let size = Size::parse(&self.prefs.video.size).unwrap_or(Size {
                        width: 3840,
                        height: 2160,
                    });
                    let (x, y, w, h) = framing::crop_rect(ts.x as u32, ts.y as u32, size, &m);
                    let r = resp.rect;
                    let sx = r.width() / ts.x;
                    let sy = r.height() / ts.y;
                    let crop = egui::Rect::from_min_size(
                        r.min + egui::vec2(x as f32 * sx, y as f32 * sy),
                        egui::vec2(w as f32 * sx, h as f32 * sy),
                    );
                    ui.painter().rect_stroke(
                        crop,
                        0.0,
                        egui::Stroke::new(1.5, GOLD),
                        egui::StrokeKind::Inside,
                    );
                }
            }
            Ok(None) => {
                ui.add_sized(max, egui::Spinner::new());
            }
            Err(e) => {
                ui.add_sized(
                    max,
                    egui::Label::new(RichText::new(format!("No preview: {e}")).color(TEXT_DIM)),
                );
            }
        }

        ui.add_space(6.0);
        egui::Grid::new("info")
            .num_columns(4)
            .spacing([14.0, 4.0])
            .show(ui, |ui| {
                let e = &f.exif;
                ui.label(RichText::new("Shutter").color(TEXT_DIM));
                ui.label(e.shutter_s.map(shutter).unwrap_or_else(|| "–".into()));
                ui.label(RichText::new("Aperture").color(TEXT_DIM));
                ui.label(
                    e.aperture
                        .map(|a| format!("f/{a:.1}"))
                        .unwrap_or_else(|| "–".into()),
                );
                ui.end_row();
                ui.label(RichText::new("ISO").color(TEXT_DIM));
                ui.label(
                    e.iso
                        .map(|v| format!("{v:.0}"))
                        .unwrap_or_else(|| "–".into()),
                );
                ui.label(RichText::new("Captured").color(TEXT_DIM));
                ui.label(e.captured.clone().unwrap_or_else(|| "–".into()));
                ui.end_row();
                ui.label(RichText::new("Holy grail").color(TEXT_DIM));
                ui.label(ev(f.holy_grail_ev));
                ui.label(RichText::new("Deflicker").color(TEXT_DIM));
                ui.label(ev(f.deflicker_ev));
                ui.end_row();
            });
        if !f.values.is_empty() {
            ui.add_space(4.0);
            ui.horizontal_wrapped(|ui| {
                for (k, v) in &f.values {
                    ui.label(
                        RichText::new(format!("{} {v:.2}", k.trim_end_matches("2012")))
                            .small()
                            .color(SKY),
                    );
                }
            });
        }
    }

    fn graph(&mut self, ui: &mut egui::Ui) {
        use egui_plot::{Corner, Legend, Line, Plot, PlotPoints, VLine};
        let Some(p) = &self.project else { return };
        let n = p.frames.len();
        let exposure: Vec<[f64; 2]> = (0..n)
            .map(|i| [i as f64, p.final_value(i, EXPOSURE).unwrap_or(0.0)])
            .collect();
        let holy: Vec<[f64; 2]> = p
            .frames
            .iter()
            .enumerate()
            .map(|(i, f)| [i as f64, f.holy_grail_ev])
            .collect();
        let defl: Vec<[f64; 2]> = p
            .frames
            .iter()
            .enumerate()
            .map(|(i, f)| [i as f64, f.deflicker_ev])
            .collect();
        // Brightness as it will look: measured brightness plus every exposure change, relative to frame 0.
        let base = p.frames.iter().find_map(|f| f.luminance);
        let bright: Vec<[f64; 2]> = match base {
            Some(b) => p
                .frames
                .iter()
                .enumerate()
                .filter(|(_, f)| !f.excluded)
                .filter_map(|(i, f)| {
                    let l = f.luminance?;
                    Some([i as f64, l - b + p.final_value(i, EXPOSURE).unwrap_or(0.0)])
                })
                .collect(),
            None => Vec::new(),
        };
        let measured: Vec<[f64; 2]> = match base {
            Some(b) => p
                .frames
                .iter()
                .enumerate()
                .filter_map(|(i, f)| Some([i as f64, f.luminance? - b]))
                .collect(),
            None => Vec::new(),
        };
        let keys = p.keyframe_indices();
        let sel = self.selected;

        let resp = Plot::new("curves")
            .legend(Legend::default().position(Corner::LeftTop))
            .allow_drag(false)
            .allow_boxed_zoom(false)
            .allow_double_click_reset(true)
            .y_axis_label("EV (stops)")
            .x_axis_label("frame")
            .include_y(-1.0)
            .include_y(1.0)
            .show(ui, |pu| {
                if !measured.is_empty() {
                    pu.line(
                        Line::new("Brightness: original", PlotPoints::from(measured))
                            .color(Color32::from_gray(140))
                            .width(1.0),
                    );
                    pu.line(
                        Line::new("Brightness: result", PlotPoints::from(bright))
                            .color(GOLD_SOFT)
                            .width(2.0),
                    );
                }
                pu.line(
                    Line::new("Exposure", PlotPoints::from(exposure))
                        .color(SKY)
                        .width(1.5),
                );
                pu.line(
                    Line::new("Holy grail", PlotPoints::from(holy))
                        .color(theme::GOOD)
                        .width(1.0),
                );
                pu.line(
                    Line::new("Deflicker", PlotPoints::from(defl))
                        .color(theme::WARN)
                        .width(1.0),
                );
                for k in &keys {
                    pu.vline(VLine::new("Keyframes", *k as f64).color(GOLD).width(1.0));
                }
                pu.vline(
                    VLine::new("Selected", sel as f64)
                        .color(Color32::WHITE)
                        .width(1.5),
                );
                pu.pointer_coordinate()
            });
        let r = &resp.response;
        if (r.clicked() || r.dragged()) && n > 0 {
            if let Some(pt) = resp.inner {
                self.selected = (pt.x.round().max(0.0) as usize).min(n - 1);
                self.scroll_to_selected = true;
            }
        }
    }

    fn status_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if let Some(job) = &self.job {
                ui.spinner();
                ui.label(RichText::new(&job.label).color(theme::TEXT).strong());
                match job.progress {
                    Some((d, t)) if t > 0 => {
                        ui.add(
                            egui::ProgressBar::new(d as f32 / t as f32)
                                .text(format!("{d} / {t}"))
                                .desired_width(260.0),
                        );
                    }
                    Some((d, _)) => {
                        // Encoding: ffmpeg reports frames written; the total is the included frames.
                        let total = self
                            .project
                            .as_ref()
                            .map(|p| p.included_indices().len())
                            .unwrap_or(0);
                        if total > 0 {
                            let f = (d as f32 / total as f32).min(1.0);
                            ui.add(
                                egui::ProgressBar::new(f)
                                    .text(format!("encoding {d} / {total}"))
                                    .desired_width(260.0),
                            );
                        } else {
                            ui.label(format!("frame {d}"));
                        }
                    }
                    None => {}
                }
                if !self.queue.is_empty() {
                    ui.label(
                        RichText::new(format!("+{} queued", self.queue.len())).color(TEXT_DIM),
                    );
                }
                if ui.button("Cancel").clicked() {
                    job.cancel();
                    self.queue.clear();
                }
            } else if let Some(last) = self.log.last() {
                ui.label(RichText::new(last).color(TEXT_DIM));
            } else if let Some(p) = &self.project {
                ui.label(
                    RichText::new(format!(
                        "{} frames · {} keyframes · {} left out",
                        p.frames.len(),
                        p.keyframe_indices().len(),
                        p.frames.len() - p.included_indices().len()
                    ))
                    .color(TEXT_DIM),
                );
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let label = if self.show_log {
                    "Hide messages"
                } else {
                    "Messages"
                };
                if ui.button(label).clicked() {
                    self.show_log = !self.show_log;
                }
                if self.project.is_some() {
                    ui.label(
                        RichText::new("↑↓ move · K keyframe · X leave out")
                            .small()
                            .color(TEXT_DIM),
                    );
                }
            });
        });
    }

    fn log_panel(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            theme::eyebrow(ui, "Messages");
            if ui.small_button("Clear").clicked() {
                self.log.clear();
            }
        });
        egui::ScrollArea::vertical()
            .stick_to_bottom(true)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for l in &self.log {
                    let c = if l.starts_with('✖') || l.starts_with("error") {
                        theme::WARN
                    } else if l.starts_with("warning") {
                        GOLD_SOFT
                    } else if l.starts_with('✔') {
                        theme::GOOD
                    } else {
                        theme::TEXT
                    };
                    ui.label(RichText::new(l).monospace().size(12.0).color(c));
                }
            });
    }
}

// ---------------------------------------------------------------- windows

impl HatApp {
    fn windows(&mut self, ctx: &egui::Context) {
        let n = self.project.as_ref().map(|p| p.frames.len()).unwrap_or(0);
        let busy = self.busy();

        let mut open = self.win.deflicker;
        centred(egui::Window::new("Deflicker"), ctx).open(&mut open).resizable(false).show(ctx, |ui| {
            let o = &mut self.prefs.deflicker;
            ui.label("Evens out frame-to-frame flicker and keeps slow changes such as a sunset.");
            ui.add(egui::Slider::new(&mut o.window, 3..=91).text("Smoothing (frames)"));
            ui.label(RichText::new("Raise for stubborn flicker; lower if passing clouds are flattened.").small().color(TEXT_DIM));
            ui.checkbox(&mut o.use_roi, "Measure only part of the frame");
            if o.use_roi {
                ui.horizontal(|ui| {
                    for (v, l) in o.roi.iter_mut().zip(["x", "y", "width", "height"]) {
                        ui.label(l);
                        ui.add(egui::DragValue::new(v).range(0.0..=1.0).speed(0.01));
                    }
                });
                ui.label(RichText::new("0–1 of the frame. 0, 0.5, 1, 0.5 = bottom half (ignores the sky).").small().color(TEXT_DIM));
            }
            ui.checkbox(&mut o.reset, "Start again (discard earlier corrections)");
            ui.separator();
            let mut base: Vec<String> = vec!["deflicker".into(), "--window".into(), o.window.to_string()];
            if o.use_roi {
                base.push("--roi".into());
                base.push(o.roi.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(","));
            }
            let reset = o.reset;
            let passes = o.passes;
            ui.add_enabled_ui(!busy, |ui| {
                if ui.add(theme::primary("Deflicker (fast)")).on_hover_text("Measures the original files").clicked() {
                    let mut a = base.clone();
                    if reset {
                        a.push("--reset".into());
                    }
                    self.queue.push_back(Step { label: "Deflicker".into(), dir: self.dir.clone(), args: a });
                }
                ui.horizontal(|ui| {
                    if ui.button("Deflicker with developed previews").on_hover_text("Most accurate: renders small previews with all your edits, measures, and repeats").clicked() {
                        for k in 0..passes {
                            self.queue.push_back(Step {
                                label: format!("Render previews (pass {})", k + 1),
                                dir: self.dir.clone(),
                                args: vec!["render".into(), "--previews".into()],
                            });
                            let mut a = base.clone();
                            a.extend(["--source".into(), "previews".into()]);
                            if reset && k == 0 {
                                a.push("--reset".into());
                            }
                            self.queue.push_back(Step { label: format!("Deflicker (pass {})", k + 1), dir: self.dir.clone(), args: a });
                        }
                    }
                    ui.add(egui::DragValue::new(&mut self.prefs.deflicker.passes).range(1..=5).suffix(" passes"));
                });
            });
        });
        self.win.deflicker = open;

        let mut open = self.win.video;
        centred(egui::Window::new("Make video"), ctx)
            .open(&mut open)
            .resizable(false)
            .show(ctx, |ui| {
                let o = &mut self.prefs.video;
                egui::Grid::new("video")
                    .num_columns(2)
                    .spacing([12.0, 8.0])
                    .show(ui, |ui| {
                        ui.label("Size");
                        combo(ui, "vsize", &mut o.size, SIZES);
                        ui.end_row();
                        if o.size == "custom" {
                            ui.label("");
                            ui.horizontal(|ui| {
                                ui.add(egui::DragValue::new(&mut o.custom[0]).range(64..=32768));
                                ui.label("×");
                                ui.add(egui::DragValue::new(&mut o.custom[1]).range(64..=32768));
                            });
                            ui.end_row();
                        }
                        ui.label("Format");
                        combo(ui, "vcodec", &mut o.codec, CODECS);
                        ui.end_row();
                        ui.label("Colour");
                        combo(ui, "vcolour", &mut o.colour, &COLOURS[..5]);
                        ui.end_row();
                        ui.label("Frame rate");
                        ui.horizontal(|ui| {
                            for f in [24.0, 25.0, 30.0, 50.0, 60.0] {
                                ui.selectable_value(&mut o.fps, f, format!("{f:.0}"));
                            }
                        });
                        ui.end_row();
                        if matches!(o.codec.as_str(), "h264" | "hevc" | "av1") {
                            ui.label("Quality");
                            ui.add(
                                egui::Slider::new(&mut o.quality, 10..=28).text("lower = better"),
                            );
                            ui.end_row();
                        }
                        ui.label("Motion blur");
                        ui.add(egui::Slider::new(&mut o.motion_blur, 1..=8).text("frames blended"));
                        ui.end_row();
                        ui.label("Developed frames");
                        ui.horizontal(|ui| {
                            let t = o
                                .from
                                .as_ref()
                                .map(|p| p.display().to_string())
                                .unwrap_or_else(|| "LightCraft render".into());
                            ui.label(RichText::new(t).color(TEXT_DIM));
                            if ui.small_button("Lightroom export…").clicked() {
                                o.from = rfd::FileDialog::new()
                                    .set_title("Folder of frames exported from Lightroom")
                                    .pick_folder();
                            }
                            if o.from.is_some() && ui.small_button("✕").clicked() {
                                o.from = None;
                            }
                        });
                        ui.end_row();
                        ui.label("Save as");
                        ui.horizontal(|ui| {
                            let t = o
                                .out
                                .as_ref()
                                .map(|p| p.display().to_string())
                                .unwrap_or_else(|| "in the sequence folder".into());
                            ui.label(RichText::new(t).color(TEXT_DIM));
                            if ui.small_button("Choose…").clicked() {
                                let ext = if o.codec.starts_with("prores")
                                    || o.codec.starts_with("dnxhr")
                                {
                                    "mov"
                                } else {
                                    "mp4"
                                };
                                o.out = rfd::FileDialog::new()
                                    .add_filter("Video", &[ext])
                                    .save_file();
                            }
                        });
                        ui.end_row();
                    });
                let hdr = o.colour.starts_with("hdr");
                if hdr && (o.codec == "h264" || o.codec == "dnxhr-hq") {
                    ui.colored_label(
                        theme::WARN,
                        "This format cannot carry HDR. Choose HEVC, AV1, ProRes or DNxHR HQX/444.",
                    );
                }
                if hdr {
                    ui.label(
                        RichText::new(
                            "For HDR, render 32-bit float first (Render → 32-bit float).",
                        )
                        .small()
                        .color(GOLD_SOFT),
                    );
                }
                ui.add_space(6.0);
                let size = size_arg(&o.size, o.custom);
                let mut a: Vec<String> = vec![
                    "video".into(),
                    "--size".into(),
                    size,
                    "--codec".into(),
                    o.codec.clone(),
                    "--colour".into(),
                    o.colour.clone(),
                    "--fps".into(),
                    o.fps.to_string(),
                    "--quality".into(),
                    o.quality.to_string(),
                    "--motion-blur".into(),
                    o.motion_blur.to_string(),
                ];
                if let Some(f) = &o.from {
                    a.extend(["--from".into(), f.display().to_string()]);
                }
                if let Some(f) = &o.out {
                    a.extend(["--out".into(), f.display().to_string()]);
                }
                if ui
                    .add_enabled(!busy, theme::primary("Make video"))
                    .clicked()
                {
                    self.queue.push_back(Step {
                        label: "Make video".into(),
                        dir: self.dir.clone(),
                        args: a,
                    });
                }
            });
        self.win.video = open;

        let mut open = self.win.export;
        centred(egui::Window::new("TIFF export"), ctx)
            .open(&mut open)
            .resizable(false)
            .show(ctx, |ui| {
                let o = &mut self.prefs.export;
                ui.label("A numbered TIFF sequence for EffectCraft, FilmCraft or any editor.");
                egui::Grid::new("export")
                    .num_columns(2)
                    .spacing([12.0, 8.0])
                    .show(ui, |ui| {
                        ui.label("Size");
                        combo(ui, "esize", &mut o.size, SIZES);
                        ui.end_row();
                        if o.size == "custom" {
                            ui.label("");
                            ui.horizontal(|ui| {
                                ui.add(egui::DragValue::new(&mut o.custom[0]).range(64..=32768));
                                ui.label("×");
                                ui.add(egui::DragValue::new(&mut o.custom[1]).range(64..=32768));
                            });
                            ui.end_row();
                        }
                        ui.label("Colour");
                        combo(ui, "ecolour", &mut o.colour, COLOURS);
                        ui.end_row();
                        ui.label("Save to");
                        ui.horizontal(|ui| {
                            let t = o
                                .out
                                .as_ref()
                                .map(|p| p.display().to_string())
                                .unwrap_or_else(|| "export/ in the sequence folder".into());
                            ui.label(RichText::new(t).color(TEXT_DIM));
                            if ui.small_button("Choose…").clicked() {
                                o.out = rfd::FileDialog::new().pick_folder();
                            }
                        });
                        ui.end_row();
                    });
                let mut a: Vec<String> = vec![
                    "export".into(),
                    "--size".into(),
                    size_arg(&o.size, o.custom),
                    "--colour".into(),
                    o.colour.clone(),
                ];
                if let Some(f) = &o.out {
                    a.extend(["--out".into(), f.display().to_string()]);
                }
                if ui
                    .add_enabled(!busy, theme::primary("Export TIFF sequence"))
                    .clicked()
                {
                    self.queue.push_back(Step {
                        label: "TIFF export".into(),
                        dir: self.dir.clone(),
                        args: a,
                    });
                }
            });
        self.win.export = open;

        let mut open = self.win.filters;
        centred(egui::Window::new("Long-term filters"), ctx).open(&mut open).resizable(false).show(ctx, |ui| {
            let o = self.prefs.filters.clone();
            let mut o2 = o.clone();
            ui.label("Leave out frames you don't want. Nothing is deleted; Reset brings every frame back.");
            ui.add_enabled_ui(!busy, |ui| {
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label("Keep frames between");
                    ui.add(egui::DragValue::new(&mut o2.from).range(0.0..=24.0).suffix(":00"));
                    ui.label("and");
                    ui.add(egui::DragValue::new(&mut o2.to).range(0.0..=24.0).suffix(":00"));
                    if ui.button("Apply").clicked() {
                        self.run_owned("Filter: time of day", self.dir.clone(), vec!["filter".into(), "time".into(), o2.from.to_string(), o2.to.to_string()]);
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("Leave out");
                    for (d, on) in WEEKDAYS.iter().zip(o2.days.iter_mut()) {
                        ui.toggle_value(on, *d);
                    }
                    if ui.button("Apply").clicked() {
                        let mut a = vec!["filter".to_string(), "weekdays".into()];
                        a.extend(WEEKDAYS.iter().zip(o2.days).filter(|(_, on)| *on).map(|(d, _)| d.to_lowercase()));
                        self.run_owned("Filter: weekdays", self.dir.clone(), a);
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("Keep one frame every");
                    ui.add(egui::TextEdit::singleline(&mut o2.period).desired_width(50.0));
                    ui.label(RichText::new("(30m, 6h, 1d, 1w)").small().color(TEXT_DIM));
                    if ui.button("Apply").clicked() {
                        self.run_owned("Filter: one per period", self.dir.clone(), vec!["filter".into(), "every".into(), o2.period.clone()]);
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("Leave out frames more than");
                    ui.add(egui::DragValue::new(&mut o2.max_ev).range(0.1..=5.0).speed(0.05).suffix(" EV"));
                    ui.label("from their neighbours");
                    if ui.button("Apply").clicked() {
                        self.run_owned(
                            "Filter: outliers",
                            self.dir.clone(),
                            vec!["filter".into(), "outliers".into(), "--window".into(), o2.outlier_window.to_string(), "--max-ev".into(), o2.max_ev.to_string()],
                        );
                    }
                });
                ui.separator();
                if ui.button("Reset: include every frame").clicked() {
                    self.run_owned("Filter: reset", self.dir.clone(), vec!["filter".into(), "reset".into()]);
                }
            });
            self.prefs.filters = o2;
        });
        self.win.filters = open;

        let mut open = self.win.motion;
        centred(egui::Window::new("Pan & zoom"), ctx).open(&mut open).resizable(false).show(ctx, |ui| {
            ui.label("Add motion keyframes; the view moves smoothly between them. The gold box on the preview shows the framing.");
            ui.horizontal(|ui| {
                ui.label("Zoom");
                ui.add(egui::DragValue::new(&mut self.motion[0]).range(1.0..=4.0).speed(0.01).suffix("×"));
                ui.label("Centre x");
                ui.add(egui::DragValue::new(&mut self.motion[1]).range(0.0..=1.0).speed(0.005));
                ui.label("y");
                ui.add(egui::DragValue::new(&mut self.motion[2]).range(0.0..=1.0).speed(0.005));
            });
            ui.add_enabled_ui(!busy && n > 0, |ui| {
                ui.horizontal(|ui| {
                    if ui.add(theme::primary(&format!("Add at frame {}", self.selected))).clicked() {
                        let [z, x, y] = self.motion;
                        self.run_owned(
                            "Add pan/zoom key",
                            self.dir.clone(),
                            vec!["motion".into(), "add".into(), self.selected.to_string(), "--zoom".into(), z.to_string(), "--cx".into(), x.to_string(), "--cy".into(), y.to_string()],
                        );
                    }
                    if ui.button("Clear all").clicked() {
                        self.run_owned("Clear pan/zoom", self.dir.clone(), vec!["motion".into(), "clear".into()]);
                    }
                });
            });
            if let Some(p) = &self.project {
                ui.separator();
                if p.motion.is_empty() {
                    ui.label(RichText::new("No motion keys: the full frame is used.").color(TEXT_DIM));
                }
                for k in &p.motion {
                    ui.label(format!("Frame {:>5}   zoom {:.2}×   centre {:.2}, {:.2}", k.frame, k.zoom, k.cx, k.cy));
                }
            }
        });
        self.win.motion = open;

        let mut open = self.win.batch;
        centred(egui::Window::new("Batch"), ctx).open(&mut open).resizable(true).default_width(520.0).show(ctx, |ui| {
            ui.label("Run the same steps over many sequences. Edit keyframes first; batch is for finished edits or long-term sequences.");
            ui.horizontal(|ui| {
                if ui.button("Add folders…").clicked() {
                    if let Some(ds) = rfd::FileDialog::new().set_title("Sequence folders, or a parent folder").pick_folders() {
                        for d in ds {
                            if !self.prefs.batch.dirs.contains(&d) {
                                self.prefs.batch.dirs.push(d);
                            }
                        }
                    }
                }
                if ui.button("Clear").clicked() {
                    self.prefs.batch.dirs.clear();
                }
            });
            egui::ScrollArea::vertical().max_height(140.0).show(ui, |ui| {
                for d in &self.prefs.batch.dirs {
                    ui.label(RichText::new(d.display().to_string()).small());
                }
            });
            ui.separator();
            ui.horizontal_wrapped(|ui| {
                ui.label("Steps:");
                for (s, on) in BATCH_STEPS.iter().zip(self.prefs.batch.steps.iter_mut()) {
                    ui.toggle_value(on, *s);
                }
            });
            ui.label(RichText::new("Video and export use the settings in the Make video and TIFF export windows.").small().color(TEXT_DIM));
            ui.horizontal(|ui| {
                ui.label("Also make");
                ui.add(egui::TextEdit::singleline(&mut self.prefs.batch.also).hint_text("e.g. prores422-hq,hevc").desired_width(200.0));
            });
            ui.checkbox(&mut self.prefs.batch.keep_going, "Carry on if a sequence fails");
            let b = &self.prefs.batch;
            let v = &self.prefs.video;
            let steps: Vec<&str> = BATCH_STEPS.iter().zip(b.steps).filter(|(_, on)| *on).map(|(s, _)| *s).collect();
            let ready = !b.dirs.is_empty() && !steps.is_empty() && !busy;
            if ui.add_enabled(ready, theme::primary("Run batch")).clicked() {
                let mut a: Vec<String> = vec!["batch".into()];
                a.extend(b.dirs.iter().map(|d| d.display().to_string()));
                a.extend(["--steps".into(), steps.join(",")]);
                a.extend(["--size".into(), size_arg(&v.size, v.custom), "--codec".into(), v.codec.clone(), "--colour".into(), v.colour.clone()]);
                a.extend(["--fps".into(), v.fps.to_string(), "--quality".into(), v.quality.to_string(), "--motion-blur".into(), v.motion_blur.to_string()]);
                if !b.also.trim().is_empty() {
                    a.extend(["--also".into(), b.also.trim().to_string()]);
                }
                if b.keep_going {
                    a.push("--keep-going".into());
                }
                self.queue.push_back(Step { label: "Batch".into(), dir: None, args: a });
                self.show_log = true;
            }
        });
        self.win.batch = open;

        let mut open = self.win.help;
        centred(egui::Window::new("How it works"), ctx).open(&mut open).default_width(560.0).show(ctx, |ui| {
            for (n, t, d) in [
                ("1", "Keyframes", "Choose the frames you'll edit by hand, then label them so your editor can find them."),
                ("", "Edit", "In LightCraft or Lightroom: set the overall look on the first keyframe and adjust the others for what changes over time. Save metadata to file (⌘S)."),
                ("2", "Holy grail", "If you changed shutter, aperture or ISO during the shoot, this cancels the jumps."),
                ("3", "Read keyframes", "Blends your keyframe edits across every frame and copies the first keyframe's look to all of them."),
                ("4", "Deflicker", "Evens out remaining flicker. The graph shows the original and resulting brightness."),
                ("5", "Render", "Develops every frame with LightCraft. Developing in Lightroom? Export there instead."),
                ("6", "Video", "H.264, HEVC, AV1, ProRes or DNxHR, up to 8K and beyond, SDR or HDR."),
                ("7", "TIFF export", "A frame sequence for EffectCraft, FilmCraft or another editor."),
            ] {
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new(format!("{n:>2}")).color(GOLD).strong());
                    ui.label(RichText::new(t).color(theme::TEXT).strong());
                    ui.label(RichText::new(d).color(TEXT_DIM));
                });
            }
            ui.separator();
            ui.label("Keys: ↑ ↓ move between frames · K toggles keyframe · X leaves a frame out. Click or drag on the graph to jump to a frame.");
            ui.hyperlink_to("Full user guide", "https://github.com/neilmanfredit/hat_timelapse/blob/main/docs/USER_GUIDE.md");
        });
        self.win.help = open;
    }
}

// ---------------------------------------------------------------- helpers

#[derive(Default)]
struct StepState {
    keyframes: usize,
    holy_grail: bool,
    interpolated: bool,
    deflickered: bool,
    rendered: usize,
    included: usize,
}

impl StepState {
    fn of(p: &Project) -> StepState {
        let rendered_dir = p.dir.join(WORK_DIR).join("rendered");
        let rendered = std::fs::read_dir(&rendered_dir)
            .map(|d| {
                d.filter_map(|e| e.ok())
                    .filter(|e| sequence::is_image(&e.path()))
                    .count()
            })
            .unwrap_or(0);
        StepState {
            keyframes: p.keyframe_indices().len(),
            holy_grail: p.frames.iter().any(|f| f.holy_grail_ev != 0.0),
            interpolated: p.frames.first().is_some_and(|f| !f.values.is_empty()),
            deflickered: p.frames.iter().any(|f| f.deflicker_ev != 0.0),
            rendered,
            included: p.included_indices().len(),
        }
    }
}

fn step_label(n: usize, name: &str, done: bool, extra: &str) -> RichText {
    let tick = if done { "✔ " } else { "" };
    let extra = if extra.is_empty() {
        String::new()
    } else {
        format!("  {extra}")
    };
    let t = RichText::new(format!("{n}  {tick}{name}{extra}"));
    if done {
        t.color(GOLD_SOFT)
    } else {
        t
    }
}

fn step_button(ui: &mut egui::Ui, n: usize, name: &str, done: bool) -> egui::Response {
    ui.button(step_label(n, name, done, ""))
}

fn step_menu(
    ui: &mut egui::Ui,
    n: usize,
    name: &str,
    done: bool,
    extra: &str,
    add: impl FnOnce(&mut egui::Ui),
) {
    ui.menu_button(step_label(n, &format!("{name} ▾"), done, extra), |ui| {
        ui.set_max_width(340.0);
        add(ui)
    });
}

fn combo(ui: &mut egui::Ui, id: &str, value: &mut String, options: &[(&str, &str)]) {
    let shown = options
        .iter()
        .find(|(k, _)| k == value)
        .map(|(_, l)| *l)
        .unwrap_or(value.as_str())
        .to_string();
    egui::ComboBox::from_id_salt(id)
        .selected_text(shown)
        .width(260.0)
        .show_ui(ui, |ui| {
            for (k, l) in options {
                ui.selectable_value(value, k.to_string(), *l);
            }
        });
}

fn size_arg(size: &str, custom: [u32; 2]) -> String {
    if size == "custom" {
        format!("{}x{}", custom[0], custom[1])
    } else {
        size.to_string()
    }
}

fn shutter(s: f64) -> String {
    if s <= 0.0 {
        String::new()
    } else if s < 0.4 {
        format!("1/{:.0}", 1.0 / s)
    } else {
        format!("{s:.1}s")
    }
}

fn ev(v: f64) -> String {
    if v.abs() < 0.005 {
        "·".into()
    } else {
        format!("{v:+.2}")
    }
}

fn folder_name(d: &Path) -> String {
    d.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| d.display().to_string())
}

fn reveal(d: &Path) {
    let cmd = if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(windows) {
        "explorer"
    } else {
        "xdg-open"
    };
    let _ = std::process::Command::new(cmd).arg(d).spawn();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formatting() {
        assert_eq!(shutter(1.0 / 2500.0), "1/2500");
        assert_eq!(shutter(2.0), "2.0s");
        assert_eq!(ev(0.0), "·");
        assert_eq!(ev(-1.234), "-1.23");
        assert_eq!(size_arg("custom", [5000, 2000]), "5000x2000");
    }

    #[test]
    fn every_option_is_accepted_by_the_engine() {
        // These must match the command line's value names.
        for (k, _) in SIZES.iter().filter(|(k, _)| *k != "custom") {
            assert!(Size::parse(k).is_ok(), "{k}");
        }
    }
}

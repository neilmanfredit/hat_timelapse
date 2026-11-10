//! Running workflow steps in the background.
//!
//! Each step runs the `hat_timelapse` command-line engine as a separate
//! process, so the app does exactly what the tested command line does, a
//! failed step can never take the app down with it, and Cancel can stop
//! any step immediately.

use std::ffi::OsString;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};

pub enum Msg {
    Line(String),
    Progress(usize, usize),
    Finished(bool),
}

pub struct Job {
    pub label: String,
    /// The step rewrites the frames' sidecars, so the editor must re-read them.
    pub writes_sidecars: bool,
    pub progress: Option<(usize, usize)>,
    pub finished: Option<bool>,
    rx: Receiver<Msg>,
    child: Arc<Mutex<Option<Child>>>,
}

/// The engine: next to this program first (as shipped), then on the PATH.
pub fn engine_path() -> PathBuf {
    let name = if cfg!(windows) {
        "hat_timelapse.exe"
    } else {
        "hat_timelapse"
    };
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let p = dir.join(name);
            if p.exists() {
                return p;
            }
        }
    }
    PathBuf::from(name)
}

/// Apps started from the Finder or a desktop launcher do not get the shell's
/// PATH, so tools installed in the usual places (Homebrew, ~/bin, Cargo) would
/// not be found. Add those places.
pub fn search_path() -> OsString {
    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).collect())
        .unwrap_or_default();
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from);
    let mut extra: Vec<PathBuf> = Vec::new();
    if let Some(h) = &home {
        extra.push(h.join("bin"));
        extra.push(h.join(".local/bin"));
        extra.push(h.join(".cargo/bin"));
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(d) = exe.parent() {
            extra.push(d.to_path_buf());
        }
    }
    for d in ["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin", "/bin"] {
        extra.push(PathBuf::from(d));
    }
    for d in extra {
        if !dirs.contains(&d) {
            dirs.push(d);
        }
    }
    std::env::join_paths(dirs).unwrap_or_default()
}

impl Job {
    /// Start `hat_timelapse -p <dir> <args…>`. `dir` is `None` for commands that
    /// take their own folders (batch, doctor).
    pub fn start(label: &str, dir: Option<&Path>, args: &[String], ctx: egui::Context) -> Job {
        let (tx, rx) = channel();
        let child_slot = Arc::new(Mutex::new(None));
        let mut cmd = Command::new(engine_path());
        if let Some(d) = dir {
            cmd.arg("-p").arg(d);
        }
        cmd.args(args)
            .env("PATH", search_path())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let _ = tx.send(Msg::Line(format!("▶ {label}")));
        match cmd.spawn() {
            Ok(mut child) => {
                let out = child.stdout.take();
                let err = child.stderr.take();
                *child_slot.lock().unwrap() = Some(child);
                let readers: Vec<_> = [
                    out.map(|o| Box::new(o) as Box<dyn Read + Send>),
                    err.map(|e| Box::new(e) as _),
                ]
                .into_iter()
                .flatten()
                .map(|r| {
                    let tx = tx.clone();
                    let ctx = ctx.clone();
                    std::thread::spawn(move || pump(r, tx, ctx))
                })
                .collect();
                let slot = child_slot.clone();
                std::thread::spawn(move || {
                    for r in readers {
                        let _ = r.join();
                    }
                    // Readers are done: the process has closed its output. Collect its status.
                    let ok = loop {
                        let mut g = slot.lock().unwrap();
                        match g.as_mut() {
                            Some(c) => match c.try_wait() {
                                Ok(Some(s)) => break s.success(),
                                Ok(None) => {}
                                Err(_) => break false,
                            },
                            None => break false, // cancelled
                        }
                        drop(g);
                        std::thread::sleep(std::time::Duration::from_millis(50));
                    };
                    let _ = tx.send(Msg::Finished(ok));
                    ctx.request_repaint();
                });
            }
            Err(e) => {
                let _ = tx.send(Msg::Line(format!(
                    "Could not start the Hat Timelapse engine ({}): {e}. Reinstall Hat Timelapse.",
                    engine_path().display()
                )));
                let _ = tx.send(Msg::Finished(false));
            }
        }
        let writes_sidecars = match args.first().map(String::as_str) {
            Some("holy-grail" | "interpolate" | "deflicker" | "write") => true,
            Some("keyframes") => args.get(1).map(String::as_str) == Some("label"),
            _ => false,
        };
        Job {
            label: label.to_string(),
            writes_sidecars,
            progress: None,
            finished: None,
            rx,
            child: child_slot,
        }
    }

    /// Collect new output. Returns the lines received.
    pub fn poll(&mut self) -> Vec<String> {
        let mut lines = Vec::new();
        while let Ok(m) = self.rx.try_recv() {
            match m {
                Msg::Line(l) => lines.push(l),
                Msg::Progress(d, t) => self.progress = Some((d, t)),
                Msg::Finished(ok) => self.finished = Some(ok),
            }
        }
        lines
    }

    pub fn cancel(&self) {
        if let Some(mut c) = self.child.lock().unwrap().take() {
            let _ = c.kill();
            let _ = c.wait();
        }
    }
}

/// Split output on newlines and carriage returns (progress counters rewrite
/// one line), and recognise "rendering 12/50" style progress.
fn pump(mut r: Box<dyn Read + Send>, tx: Sender<Msg>, ctx: egui::Context) {
    let mut buf = [0u8; 4096];
    let mut line = Vec::new();
    let mut reader = BufReader::new(&mut r);
    loop {
        let n = match reader.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => n,
        };
        for &b in &buf[..n] {
            if b == b'\n' || b == b'\r' {
                if !line.is_empty() {
                    emit(&String::from_utf8_lossy(&line), &tx);
                    line.clear();
                }
            } else {
                line.push(b);
            }
        }
        ctx.request_repaint();
    }
    if !line.is_empty() {
        emit(&String::from_utf8_lossy(&line), &tx);
    }
    ctx.request_repaint();
}

fn emit(line: &str, tx: &Sender<Msg>) {
    let t = line.trim();
    if let Some(p) = parse_progress(t) {
        let _ = tx.send(Msg::Progress(p.0, p.1));
        return;
    }
    // ffmpeg's running status ("frame=  120 fps=…") is progress, not log.
    if t.starts_with("frame=") {
        if let Some(n) = t
            .trim_start_matches("frame=")
            .split_whitespace()
            .next()
            .and_then(|x| x.parse().ok())
        {
            let _ = tx.send(Msg::Progress(n, 0));
        }
        return;
    }
    let _ = tx.send(Msg::Line(t.to_string()));
}

/// "rendering 12/50" or "framing 12/50" → (12, 50)
pub fn parse_progress(t: &str) -> Option<(usize, usize)> {
    let rest = t
        .strip_prefix("rendering ")
        .or_else(|| t.strip_prefix("framing "))?;
    let (a, b) = rest.split_once('/')?;
    Some((a.trim().parse().ok()?, b.trim().parse().ok()?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress() {
        assert_eq!(parse_progress("rendering 12/50"), Some((12, 50)));
        assert_eq!(parse_progress("framing 3/60"), Some((3, 60)));
        assert_eq!(parse_progress("rendered 12 of 50"), None);
    }

    #[test]
    fn search_path_adds_common_tool_folders() {
        let p = search_path();
        let dirs: Vec<PathBuf> = std::env::split_paths(&p).collect();
        assert!(dirs.contains(&PathBuf::from("/usr/local/bin")));
    }
}

//! Loading frame previews off the UI thread.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};

const EDGE: u32 = 1400;
const KEEP: usize = 48;

pub struct Preview {
    tx: Sender<PathBuf>,
    rx: Receiver<(PathBuf, Result<egui::ColorImage, String>)>,
    textures: HashMap<PathBuf, egui::TextureHandle>,
    order: Vec<PathBuf>,
    errors: HashMap<PathBuf, String>,
    pending: Option<PathBuf>,
}

impl Preview {
    pub fn new(ctx: &egui::Context) -> Self {
        let (tx, worker_rx) = channel::<PathBuf>();
        let (worker_tx, rx) = channel();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            while let Ok(mut path) = worker_rx.recv() {
                // Skip to the newest request when scrolling quickly.
                while let Ok(p) = worker_rx.try_recv() {
                    path = p;
                }
                let r = hat_timelapse_core::luminance::load_scaled(&path, EDGE)
                    .map(|img| {
                        let rgba = img.to_rgba8();
                        egui::ColorImage::from_rgba_unmultiplied(
                            [rgba.width() as usize, rgba.height() as usize],
                            rgba.as_raw(),
                        )
                    })
                    .map_err(|e| format!("{e:#}"));
                if worker_tx.send((path, r)).is_err() {
                    break;
                }
                ctx.request_repaint();
            }
        });
        Preview {
            tx,
            rx,
            textures: HashMap::new(),
            order: Vec::new(),
            errors: HashMap::new(),
            pending: None,
        }
    }

    /// The texture for `path`, requesting it if needed.
    pub fn get(
        &mut self,
        ctx: &egui::Context,
        path: &PathBuf,
    ) -> Result<Option<egui::TextureHandle>, String> {
        while let Ok((p, r)) = self.rx.try_recv() {
            if self.pending.as_ref() == Some(&p) {
                self.pending = None;
            }
            match r {
                Ok(img) => {
                    let tex =
                        ctx.load_texture(p.to_string_lossy(), img, egui::TextureOptions::LINEAR);
                    self.textures.insert(p.clone(), tex);
                    self.order.push(p);
                    if self.order.len() > KEEP {
                        let old = self.order.remove(0);
                        self.textures.remove(&old);
                    }
                }
                Err(e) => {
                    self.errors.insert(p, e);
                }
            }
        }
        if let Some(t) = self.textures.get(path) {
            return Ok(Some(t.clone()));
        }
        if let Some(e) = self.errors.get(path) {
            return Err(e.clone());
        }
        if self.pending.as_ref() != Some(path) {
            self.pending = Some(path.clone());
            let _ = self.tx.send(path.clone());
        }
        Ok(None)
    }

    /// Forget cached images (after a step changes what is on disk).
    pub fn clear(&mut self) {
        self.textures.clear();
        self.order.clear();
        self.errors.clear();
        self.pending = None;
    }
}

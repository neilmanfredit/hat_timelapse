//! Hat Timelapse rendering: develop frames with LightCraft, frame them (crop, pan,
//! zoom, scale, colour space) into a numbered TIFF sequence, and encode video.

pub mod colour;
pub mod framing;
pub mod lightcraft;
pub mod video;

/// Run `f` on a dedicated pool with `jobs` threads (0 = all cores).
pub fn with_jobs<R: Send>(jobs: usize, f: impl FnOnce() -> R + Send) -> anyhow::Result<R> {
    let pool = rayon::ThreadPoolBuilder::new().num_threads(jobs).build()?;
    Ok(pool.install(f))
}

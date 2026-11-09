//! Finding the frames of a sequence on disk.

use std::path::{Path, PathBuf};

pub const RAW_EXTS: &[&str] = &[
    "dng", "cr2", "cr3", "crw", "nef", "nrw", "arw", "srf", "sr2", "orf", "rw2", "rwl", "pef",
    "raf", "srw", "3fr", "iiq", "erf", "kdc", "mos", "mrw", "x3f",
];
pub const RASTER_EXTS: &[&str] = &["jpg", "jpeg", "tif", "tiff", "png", "heic", "heif"];

pub fn is_image(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| {
            let e = e.to_ascii_lowercase();
            RAW_EXTS.contains(&e.as_str()) || RASTER_EXTS.contains(&e.as_str())
        })
        .unwrap_or(false)
}

pub fn is_raw(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| RAW_EXTS.contains(&e.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

/// Image files directly inside `dir`, in natural filename order
/// (`IMG_9.CR2` before `IMG_10.CR2`). Hidden files are skipped.
pub fn scan(dir: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_file() && is_image(p))
        .filter(|p| {
            !p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with('.'))
        })
        .collect();
    files.sort_by(|a, b| {
        natural_cmp(
            &a.file_name().unwrap().to_string_lossy(),
            &b.file_name().unwrap().to_string_lossy(),
        )
    });
    Ok(files)
}

/// Compare strings treating runs of digits as numbers.
pub fn natural_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    let (mut a, mut b) = (a.as_bytes(), b.as_bytes());
    loop {
        match (a.first(), b.first()) {
            (None, None) => return Ordering::Equal,
            (None, _) => return Ordering::Less,
            (_, None) => return Ordering::Greater,
            (Some(x), Some(y)) if x.is_ascii_digit() && y.is_ascii_digit() => {
                let la = a.iter().take_while(|c| c.is_ascii_digit()).count();
                let lb = b.iter().take_while(|c| c.is_ascii_digit()).count();
                let (na, nb) = (trim_zeros(&a[..la]), trim_zeros(&b[..lb]));
                let o = na
                    .len()
                    .cmp(&nb.len())
                    .then_with(|| na.cmp(nb))
                    .then_with(|| la.cmp(&lb));
                if o != Ordering::Equal {
                    return o;
                }
                a = &a[la..];
                b = &b[lb..];
            }
            (Some(x), Some(y)) => {
                let o = x.to_ascii_lowercase().cmp(&y.to_ascii_lowercase());
                if o != Ordering::Equal {
                    return o;
                }
                a = &a[1..];
                b = &b[1..];
            }
        }
    }
}

fn trim_zeros(s: &[u8]) -> &[u8] {
    let n = s.iter().take_while(|&&c| c == b'0').count();
    &s[n.min(s.len().saturating_sub(1))..]
}

/// Lightroom-style sidecar path: `IMG_0001.CR2` → `IMG_0001.xmp`.
pub fn sidecar_path(image: &Path) -> PathBuf {
    image.with_extension("xmp")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn natural_order() {
        let mut v = vec!["IMG_10.CR2", "img_9.cr2", "IMG_100.CR2", "IMG_009.CR2"];
        v.sort_by(|a, b| natural_cmp(a, b));
        assert_eq!(
            v,
            vec!["img_9.cr2", "IMG_009.CR2", "IMG_10.CR2", "IMG_100.CR2"]
        );
    }
}

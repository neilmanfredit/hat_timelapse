//! Where each keyframe stands: not yet edited, edited and applied, or edited
//! since Hat Timelapse last applied the edits.
//!
//! Changes are detected from file contents, not timestamps: sequences often
//! live on network drives whose clocks differ from the computer's.

use crate::project::Project;
use crate::sequence::sidecar_path;
use std::path::Path;

/// FNV-1a: small, fast and stable across builds (unlike `DefaultHasher`).
pub fn fingerprint(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

pub fn fingerprint_file(path: &Path) -> Option<u64> {
    std::fs::read(path).ok().map(|b| fingerprint(&b))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyStatus {
    /// No edits saved from an editor yet.
    NotEdited,
    /// Edited, and those edits have been applied to the sequence.
    Applied,
    /// Saved from an editor since Hat Timelapse last applied the edits.
    Changed,
}

/// Does this sidecar hold develop settings (Lightroom `crs:` or LightCraft `lc:settings`)?
fn has_settings(bytes: &[u8]) -> bool {
    let s = String::from_utf8_lossy(bytes);
    s.contains("crs:") || s.contains("lc:settings")
}

pub fn keyframe_status(p: &Project, i: usize) -> KeyStatus {
    let f = &p.frames[i];
    let Ok(bytes) = std::fs::read(sidecar_path(&p.frame_path(i))) else {
        return KeyStatus::NotEdited;
    };
    match f.sidecar_fp {
        Some(fp) if fp == fingerprint(&bytes) => {
            if f.editor_saved {
                KeyStatus::Applied
            } else {
                KeyStatus::NotEdited
            }
        }
        _ if has_settings(&bytes) => KeyStatus::Changed,
        _ => KeyStatus::NotEdited,
    }
}

/// (edited and applied, changed since last apply, total keyframes)
pub fn summary(p: &Project) -> (usize, usize, usize) {
    let keys = p.keyframe_indices();
    let (mut applied, mut changed) = (0, 0);
    for &i in &keys {
        match keyframe_status(p, i) {
            KeyStatus::Applied => applied += 1,
            KeyStatus::Changed => changed += 1,
            KeyStatus::NotEdited => {}
        }
    }
    (applied, changed, keys.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingerprint_is_stable() {
        assert_eq!(fingerprint(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fingerprint(b"a"), 0xaf63_dc4c_8601_ec8c);
        assert_ne!(fingerprint(b"ab"), fingerprint(b"ba"));
    }
}

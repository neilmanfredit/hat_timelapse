//! Hat Timelapse core: the sequence model and the maths behind a timelapse workflow.
//!
//! Nothing in this crate touches a renderer or an encoder. It reads capture
//! metadata, holds the project state and works out per-frame develop values:
//! keyframe interpolation, holy-grail exposure compensation, deflicker and
//! long-term frame selection.

pub mod deflicker;
pub mod exif;
pub mod holygrail;
pub mod interp;
pub mod longterm;
pub mod luminance;
pub mod params;
pub mod project;
pub mod sequence;
pub mod status;

pub use project::{Frame, MotionKey, Project, Settings};

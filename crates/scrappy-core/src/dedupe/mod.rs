//! Dedupe: finds near-duplicate images in the library by perceptual hash, keeps the best copy and moves the others to
//! the system trash (ADR-0009).

mod fingerprint;
mod group;
mod scan;

pub use fingerprint::{fingerprint, jpeg_quality};
pub use group::{Picture, Strictness, groups};
pub use scan::{DuplicateRequest, Duplicates, remove_copies, set_auto_strictness, trash};

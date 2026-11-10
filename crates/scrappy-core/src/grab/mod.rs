//! Grab: finds everything downloadable on a page (media requests, HLS/DASH streams, images, files) and downloads the
//! picked finds into `Grabs/<domain>/<date>/` (ADR-0007). A page the extension hands over waits in the `Inbox`.

mod find;
mod inbox;
mod save;
mod scan;

pub use find::{Find, FindKind, MAX_FINDS, Seen, classify, collect};
pub use inbox::Inbox;
pub use save::{GrabRequest, Pick, Picked, save};
pub use scan::{PageScan, scan};

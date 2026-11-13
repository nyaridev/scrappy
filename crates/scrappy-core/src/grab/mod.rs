//! Grab: finds everything downloadable on a site's pages (media requests, HLS/DASH streams, images, files) with a
//! browser, and downloads the picked finds, or each find as it's found, into `Grabs/<domain>/<date>/` (ADR-0007,
//! ADR-0031). Each find's thumbnail is made when the window shows it. A page the extension hands over waits
//! in the `Inbox`.

mod find;
mod inbox;
mod keep;
mod save;
mod scan;

pub use find::{Find, FindKind, MAX_FINDS, Seen, classify, collect, thumbnail};
pub use inbox::Inbox;
pub use keep::Kept;
pub use save::{GrabRequest, Pick, Picked, save};
pub use scan::{PageScan, ScanRequest, Scans};

//! Library: the download folder the user picked, its folder layout, path templates, the catalog in
//! `<root>/.scrappy/library.db` and what it says has been downloaded (ADR-0004).

mod catalog;
mod folder;
mod gallery;
mod history;
mod images;
pub mod template;

pub use catalog::{Record, record};
pub use folder::{FolderCheck, LibraryStatus, check, setup, status};
pub use gallery::{GalleryItem, GalleryPage, GalleryQuery, GallerySort, page, picture};
pub use images::{ImageRow, forget, images, set_fingerprint};
pub use history::{Day, RecentDownload, Stats, Tally, locate, recent, sources, stats, today};

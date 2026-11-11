//! Library: the download folder the user picked, its folder layout, path templates, the catalog in
//! `<root>/.scrappy/library.db` and what it says has been downloaded (ADR-0004), and the folder as it is on disk for
//! the Gallery (ADR-0015).

mod catalog;
mod disk;
mod folder;
mod gallery;
mod history;
mod images;
pub mod template;

pub use catalog::{Record, record};
pub use disk::{Disk, OnChange};
pub use folder::{FolderCheck, LibraryStatus, check, setup, status};
pub use gallery::{GalleryItem, GalleryPage, GalleryQuery, GallerySort, page, picture};
pub use history::{Day, RecentDownload, Stats, Tally, locate, recent, sources, stats, today};
pub use images::{ImageRow, forget, images, set_fingerprint};

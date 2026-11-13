//! Library: the download folder the user picked, its folder layout, path templates, the catalog in
//! `<root>/.scrappy/library.db` and what it says has been downloaded (ADR-0004), and the folder as it is on disk for
//! the Gallery (ADR-0015), where a file or a downloaded folder can be deleted (ADR-0022) and videos and audio stream
//! to the player (ADR-0023).

mod catalog;
mod disk;
mod folder;
mod gallery;
mod history;
mod images;
mod relocate;
mod remove;
mod serve;
pub mod template;

pub use catalog::{Record, record};
pub use disk::{Disk, OnChange};
pub use folder::{FolderCheck, LibraryStatus, check, setup, status};
pub use gallery::{GalleryFolder, GalleryItem, GalleryPage, GalleryQuery, GallerySort, page, picture};
pub use history::{Day, RecentDownload, Stats, Tally, locate, recent, sources, stats, today};
pub use images::{ImageRow, forget, images, set_fingerprints};
pub use relocate::{check_move, start_move};
pub use remove::remove;
pub use serve::{MediaView, Served, serve};

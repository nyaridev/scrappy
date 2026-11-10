//! Media: drives yt-dlp to probe and download, and ffprobe and FFmpeg to check and encode with the output presets
//! (ADR-0002, ADR-0003), saves playlists, channels, posts and profiles as one job each (gallery-dl for the sites
//! `route` names), fetches single files (images the extension sends, what Grab picks), and makes the Gallery's
//! thumbnails.

mod download;
mod downloads;
mod engine;
mod gallery;
mod file;
mod inspect;
mod playlist;
mod presets;
mod probe;
mod route;
mod thumbnail;

pub use downloads::{DownloadEntry, DownloadRequest, DownloadState, Downloads, SaveRequest, SendError};
pub use presets::{PresetInfo, presets};
pub use probe::Probe;
pub use thumbnail::Thumbnails;

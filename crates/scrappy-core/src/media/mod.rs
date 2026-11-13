//! Media: drives yt-dlp to probe and download, and ffprobe and FFmpeg to check and encode with the output presets
//! (ADR-0002, ADR-0003), saves playlists, channels, posts and profiles as one job each (gallery-dl for the sites
//! `route` names), fetches single files (images the extension sends, what Grab picks), and makes the Gallery's
//! thumbnails.

mod download;
mod download_list;
mod downloads;
mod engine;
mod gallery;
mod file;
mod inspect;
mod listing;
mod playlist;
mod presets;
mod probe;
mod probes;
mod route;
mod thumbnail;

pub use download::example_name;
pub use downloads::{
    DownloadEntry, DownloadRequest, DownloadSource, DownloadState, Downloads, Row, SaveRequest, SendError, SentFrom,
};
pub use engine::set_media_options;
pub use presets::{PresetInfo, presets};
pub use probe::{ListEntry, Probe, Section};
pub use probes::Probes;
pub use thumbnail::Thumbnails;

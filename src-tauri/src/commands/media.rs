//! Media commands: probing a link, the presets, and starting, listing and removing downloads.

use std::sync::Arc;

use scrappy_core::jobs::{JobId, Jobs};
use scrappy_core::media::{self, DownloadEntry, DownloadRequest, Downloads, PresetInfo, Probe};
use scrappy_core::settings::Store;
use tauri::State;

/// A link's details, before it's downloaded.
#[tauri::command]
pub async fn probe_url(downloads: State<'_, Downloads>, url: String) -> Result<Probe, String> {
    downloads.probe(&url).await
}

/// The thumbnail of a video a probe listed, as a `data:` URL.
#[tauri::command]
pub async fn entry_thumbnail(downloads: State<'_, Downloads>, url: String) -> Result<Option<String>, String> {
    Ok(downloads.preview(&url).await)
}

/// The size a probed video will download with a preset and height, in bytes; `None` when it isn't known.
#[tauri::command]
pub async fn download_size(
    downloads: State<'_, Downloads>,
    url: String,
    preset: String,
    max_height: Option<u32>,
) -> Result<Option<u64>, String> {
    downloads.size(&url, &preset, max_height).await
}

/// The output presets, the default first.
#[tauri::command]
pub fn presets() -> Vec<PresetInfo> {
    media::presets()
}

/// Queues a download into the library (one per picked section); progress arrives as job events with each entry's id.
#[tauri::command]
pub async fn start_download(
    downloads: State<'_, Downloads>,
    jobs: State<'_, Arc<Jobs>>,
    settings: State<'_, Store>,
    request: DownloadRequest,
) -> Result<Vec<DownloadEntry>, String> {
    downloads.start(&jobs, settings.get().library_root, request).await
}

/// The kept downloads.
#[tauri::command(async)]
pub fn downloads(downloads: State<Downloads>) -> Vec<DownloadEntry> {
    downloads.list()
}

/// Removes a finished download from the list; its files stay. Returns the list.
#[tauri::command(async)]
pub fn remove_download(downloads: State<Downloads>, id: JobId) -> Result<Vec<DownloadEntry>, String> {
    downloads.remove(id)
}

/// Removes every finished download from the list; their files stay. Returns what's left.
#[tauri::command(async)]
pub fn clear_downloads(downloads: State<Downloads>) -> Vec<DownloadEntry> {
    downloads.clear()
}

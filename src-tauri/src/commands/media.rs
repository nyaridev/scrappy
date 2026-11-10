//! Media commands: probing a link, the presets, and starting and listing downloads.

use std::sync::Arc;

use scrappy_core::jobs::Jobs;
use scrappy_core::media::{self, DownloadEntry, DownloadRequest, Downloads, PresetInfo, Probe};
use scrappy_core::settings::Store;
use tauri::State;

/// A link's details, before it's downloaded.
#[tauri::command]
pub async fn probe_url(downloads: State<'_, Downloads>, url: String) -> Result<Probe, String> {
    downloads.probe(&url).await
}

/// The output presets, the default first.
#[tauri::command]
pub fn presets() -> Vec<PresetInfo> {
    media::presets()
}

/// Queues a download into the library; progress arrives as job events with the entry's id.
#[tauri::command]
pub async fn start_download(
    downloads: State<'_, Downloads>,
    jobs: State<'_, Arc<Jobs>>,
    settings: State<'_, Store>,
    request: DownloadRequest,
) -> Result<DownloadEntry, String> {
    downloads.start(&jobs, settings.get().library_root, request).await
}

/// This session's downloads.
#[tauri::command]
pub fn downloads(downloads: State<Downloads>) -> Vec<DownloadEntry> {
    downloads.list()
}

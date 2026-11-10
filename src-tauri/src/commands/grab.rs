//! Grab commands: scanning a page, downloading the picked finds, and taking the page the extension handed over.

use std::sync::Arc;

use scrappy_core::grab::{self, GrabRequest, Inbox, PageScan, Picked};
use scrappy_core::jobs::Jobs;
use scrappy_core::media::Downloads;
use scrappy_core::settings::Store;
use tauri::State;

/// Scans a page in a headless browser and lists what can be downloaded.
#[tauri::command]
pub async fn scan_page(url: String) -> Result<PageScan, String> {
    grab::scan(&url).await
}

/// Queues the picked finds into `Grabs/`; progress arrives as job events with each pick's id.
#[tauri::command]
pub async fn grab_finds(
    downloads: State<'_, Downloads>,
    jobs: State<'_, Arc<Jobs>>,
    settings: State<'_, Store>,
    request: GrabRequest,
) -> Result<Vec<Picked>, String> {
    grab::save(&downloads, &jobs, settings.library_root()?, request).await
}

/// The page the extension handed to Grab, once.
#[tauri::command]
pub fn handed_page(inbox: State<Inbox>) -> Option<PageScan> {
    inbox.take()
}

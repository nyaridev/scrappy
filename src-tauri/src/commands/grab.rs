//! Grab commands: scanning a page, a find's thumbnail, downloading the picked finds, and taking the page the extension
//! handed over.

use std::sync::Arc;

use scrappy_core::grab::{self, FindKind, GrabRequest, Inbox, PageScan, Picked, ScanRequest, Scans};
use scrappy_core::jobs::{JobId, Jobs};
use scrappy_core::media::{Downloads, Thumbnails};
use scrappy_core::settings::Store;
use tauri::State;

/// Starts scanning a page (and, deeper, the pages it links to) in a headless browser, saving finds as they're found
/// when asked; its progress arrives as job events with the id.
#[tauri::command]
pub async fn start_scan(
    scans: State<'_, Scans>,
    jobs: State<'_, Arc<Jobs>>,
    downloads: State<'_, Downloads>,
    settings: State<'_, Store>,
    request: ScanRequest,
) -> Result<JobId, String> {
    scans.start(&jobs, &downloads, settings.get().library_root, request)
}

/// What a scan has found so far; all of it once its job ends.
#[tauri::command]
pub fn scanned_page(scans: State<Scans>, id: JobId) -> Option<PageScan> {
    scans.page(id)
}

/// A find's thumbnail as a `data:` URL; `None` for audio and files, or one that can't be read.
#[tauri::command]
pub async fn find_thumbnail(
    thumbnails: State<'_, Thumbnails>,
    url: String,
    kind: FindKind,
) -> Result<Option<String>, String> {
    Ok(grab::thumbnail(&thumbnails, &url, kind).await)
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

//! Scrape commands: mirroring a site or saving a page, and a mirror's images and videos to pick from (ADR-0033).

use std::sync::Arc;

use scrappy_core::crawl::{self, CrawlRequest, CrawlStarted, MediaRequest, MirrorMedia, Mirrors};
use scrappy_core::jobs::{JobId, Jobs};
use scrappy_core::media::Downloads;
use scrappy_core::settings::Store;
use tauri::State;

/// Starts a crawl; progress arrives as job events with the returned id.
#[tauri::command]
pub async fn crawl_site(
    downloads: State<'_, Downloads>,
    jobs: State<'_, Arc<Jobs>>,
    mirrors: State<'_, Mirrors>,
    settings: State<'_, Store>,
    request: CrawlRequest,
) -> Result<CrawlStarted, String> {
    crawl::crawl(&downloads, &jobs, &mirrors, settings.library_root()?, request)
}

/// A mirror's images and videos, and how saving each went.
#[tauri::command(async)]
pub fn mirror_media(
    mirrors: State<'_, Mirrors>,
    settings: State<'_, Store>,
    site: String,
) -> Result<MirrorMedia, String> {
    mirrors.media(&settings.library_root()?, &site)
}

/// Starts saving a mirror's picked media; progress arrives as job events with the returned id.
#[tauri::command]
pub async fn save_mirror_media(
    downloads: State<'_, Downloads>,
    jobs: State<'_, Arc<Jobs>>,
    mirrors: State<'_, Mirrors>,
    settings: State<'_, Store>,
    request: MediaRequest,
) -> Result<JobId, String> {
    crawl::save_media(&mirrors, &downloads, &jobs, settings.library_root()?, request)
}

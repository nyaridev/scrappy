//! Scrape commands: crawling a site for its files.

use std::sync::Arc;

use scrappy_core::crawl::{self, CrawlRequest, CrawlStarted};
use scrappy_core::jobs::Jobs;
use scrappy_core::media::Downloads;
use scrappy_core::settings::Store;
use tauri::State;

/// Starts a crawl; progress arrives as job events with the returned id.
#[tauri::command]
pub async fn crawl_site(
    downloads: State<'_, Downloads>,
    jobs: State<'_, Arc<Jobs>>,
    settings: State<'_, Store>,
    request: CrawlRequest,
) -> Result<CrawlStarted, String> {
    crawl::crawl(&downloads, &jobs, settings.library_root()?, request)
}

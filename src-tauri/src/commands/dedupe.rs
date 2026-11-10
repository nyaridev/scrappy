//! Duplicate commands: scanning the library for near-duplicate pictures, taking the groups found, and moving copies
//! to the system trash.

use std::sync::Arc;

use scrappy_core::dedupe::{self, DuplicateRequest, Duplicates, Picture};
use scrappy_core::jobs::{JobId, Jobs};
use scrappy_core::settings::Store;
use tauri::State;

/// Starts a duplicate scan; progress arrives as job events with the returned id.
#[tauri::command]
pub async fn find_duplicates(
    duplicates: State<'_, Duplicates>,
    jobs: State<'_, Arc<Jobs>>,
    settings: State<'_, Store>,
    request: DuplicateRequest,
) -> Result<JobId, String> {
    Ok(duplicates.start(&jobs, settings.library_root()?, request))
}

/// The groups a finished scan found, best copy first; once.
#[tauri::command]
pub fn duplicates_found(duplicates: State<Duplicates>, id: JobId) -> Option<Vec<Vec<Picture>>> {
    duplicates.take(id)
}

/// Moves library files to the system trash and drops them from the catalog; returns how many moved.
#[tauri::command]
pub fn trash_files(settings: State<Store>, paths: Vec<String>) -> Result<u32, String> {
    dedupe::trash(&settings.library_root()?, &paths)
}

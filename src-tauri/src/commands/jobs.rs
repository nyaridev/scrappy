//! Job commands: cancelling a running job. Progress arrives as events (ADR-0010).

use std::sync::Arc;

use scrappy_core::jobs::{JobId, Jobs};
use tauri::State;

/// Asks a job to stop; false when it isn't running.
#[tauri::command]
pub fn cancel_job(jobs: State<Arc<Jobs>>, id: JobId) -> bool {
    jobs.cancel(id)
}

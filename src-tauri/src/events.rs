//! Events: forwards core's job reports to the window as `job-progress` and `job-finished` (ADR-0010).

use std::sync::Arc;

use scrappy_core::jobs::{JobEvent, Reporter};
use tauri::{AppHandle, Emitter};

/// A reporter that emits each job report to the window.
pub fn job_reporter(app: AppHandle) -> Reporter {
    Arc::new(move |event| {
        // Emitting only fails once the window is gone, when nobody is left to tell.
        let _ = match event {
            JobEvent::Progress(progress) => app.emit("job-progress", progress),
            JobEvent::Finished(finished) => app.emit("job-finished", finished),
        };
    })
}

//! App commands: facts about the running build, and its own updates (ADR-0037).

use scrappy_core::app::{self, AppInfo, UpdateStatus, WindowFrame};
use tauri::{AppHandle, State};

use crate::update::{self, Updates};

/// The running version, platform and window frame.
#[tauri::command]
pub fn app_info(frame: State<WindowFrame>) -> AppInfo {
    app::info(*frame)
}

/// What the updater knows now.
#[tauri::command]
pub fn update_status(updates: State<Updates>) -> UpdateStatus {
    updates.status()
}

/// Looks for a newer release now.
#[tauri::command]
pub async fn check_for_update(app: AppHandle) -> UpdateStatus {
    update::check(&app).await
}

/// Installs the update a check found and restarts Scrappy.
#[tauri::command]
pub async fn install_update(app: AppHandle) -> Result<(), String> {
    update::install(&app).await
}

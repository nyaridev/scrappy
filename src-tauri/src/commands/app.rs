//! App commands: facts about the running build.

use scrappy_core::app::{self, AppInfo};

/// The running version and platform.
#[tauri::command]
pub fn app_info() -> AppInfo {
    app::info()
}

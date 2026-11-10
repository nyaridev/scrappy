//! App commands: facts about the running build.

use scrappy_core::app::{self, AppInfo, WindowFrame};
use tauri::State;

/// The running version, platform and window frame.
#[tauri::command]
pub fn app_info(frame: State<WindowFrame>) -> AppInfo {
    app::info(*frame)
}

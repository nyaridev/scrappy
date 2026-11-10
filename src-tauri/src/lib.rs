//! The Tauri shell: opens the window, starts logging and registers the commands. All logic is in scrappy-core.

mod commands;

use tauri::Manager;

/// Starts the app.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            scrappy_core::logging::init(&app.path().app_log_dir()?)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![commands::app::app_info])
        .run(tauri::generate_context!())
        .expect("error while running Scrappy");
}

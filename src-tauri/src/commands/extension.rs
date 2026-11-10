//! Extension commands: which browsers reach this app, and connecting or disconnecting them (ADR-0014).

use std::path::PathBuf;

use scrappy_core::bridge::{self, HostStatus};
use tauri::{AppHandle, Manager};

/// The running app's folder and its data folder.
fn folders(app: &AppHandle) -> Result<(PathBuf, PathBuf), String> {
    let exe = std::env::current_exe().map_err(|e| format!("Can't find the app: {e}"))?;
    let dir = exe.parent().map(PathBuf::from).ok_or("Can't find the app's folder.")?;
    let data = app.path().app_data_dir().map_err(|e| format!("Can't find the data folder: {e}"))?;
    Ok((dir, data))
}

/// Which browsers start this app's host.
#[tauri::command]
pub fn host_status(app: AppHandle) -> Result<HostStatus, String> {
    let (dir, data) = folders(&app)?;
    Ok(bridge::host_status(&dir, &data))
}

/// Connects every installed browser to this app's host.
#[tauri::command]
pub fn connect_browsers(app: AppHandle) -> Result<HostStatus, String> {
    let (dir, data) = folders(&app)?;
    bridge::connect(&dir, &data)
}

/// Disconnects every browser.
#[tauri::command]
pub fn disconnect_browsers(app: AppHandle) -> Result<HostStatus, String> {
    let (dir, data) = folders(&app)?;
    Ok(bridge::disconnect(&dir, &data))
}

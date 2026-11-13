//! Extension commands: which browsers reach this app, connecting or disconnecting them (ADR-0014), and the
//! extension's files for this version, fetched from its release and opened in the file manager (ADR-0040).

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
#[tauri::command(async)]
pub fn host_status(app: AppHandle) -> Result<HostStatus, String> {
    let (dir, data) = folders(&app)?;
    Ok(bridge::host_status(&dir, &data))
}

/// Connects every installed browser to this app's host.
#[tauri::command(async)]
pub fn connect_browsers(app: AppHandle) -> Result<HostStatus, String> {
    let (dir, data) = folders(&app)?;
    bridge::connect(&dir, &data)
}

/// Disconnects every browser.
#[tauri::command(async)]
pub fn disconnect_browsers(app: AppHandle) -> Result<HostStatus, String> {
    let (dir, data) = folders(&app)?;
    Ok(bridge::disconnect(&dir, &data))
}

/// The folder with the extension for this version, once it's been downloaded.
#[tauri::command(async)]
pub fn extension_files(app: AppHandle) -> Result<Option<String>, String> {
    let (_, data) = folders(&app)?;
    let folder = bridge::extension_files(&data, &app.package_info().version.to_string());
    Ok(folder.map(|folder| folder.to_string_lossy().into_owned()))
}

/// Downloads the extension for this version when it isn't there yet, then opens its folder. Returns the folder.
#[tauri::command]
pub async fn open_extension_files(app: AppHandle) -> Result<String, String> {
    let (_, data) = folders(&app)?;
    let folder = bridge::fetch_extension(&data, &app.package_info().version.to_string()).await?;
    tauri_plugin_opener::open_path(&folder, None::<&str>).map_err(|e| format!("Can't open the folder: {e}"))?;
    Ok(folder.to_string_lossy().into_owned())
}

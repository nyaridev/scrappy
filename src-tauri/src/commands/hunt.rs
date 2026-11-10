//! Hunt commands: the source packs, starting a hunt, and the user's packs folder.

use std::path::PathBuf;
use std::sync::Arc;

use scrappy_core::hunt::{self, HuntRequest, HuntStarted, Packs};
use scrappy_core::jobs::Jobs;
use scrappy_core::media::Downloads;
use scrappy_core::settings::Store;
use tauri::{AppHandle, Manager, State};

/// The user's packs folder, made if it's missing.
fn packs_folder(app: &AppHandle) -> Result<PathBuf, String> {
    let config = app.path().app_config_dir().map_err(|e| format!("Can't find the config folder: {e}"))?;
    hunt::packs_folder(&config)
}

/// The built-in packs and the user's.
#[tauri::command]
pub fn hunt_packs(app: AppHandle) -> Packs {
    hunt::packs(packs_folder(&app).ok().as_deref())
}

/// Starts a hunt; progress arrives as job events with the returned id.
#[tauri::command]
pub async fn start_hunt(
    app: AppHandle,
    downloads: State<'_, Downloads>,
    jobs: State<'_, Arc<Jobs>>,
    settings: State<'_, Store>,
    request: HuntRequest,
) -> Result<HuntStarted, String> {
    let folder = packs_folder(&app)?;
    hunt::hunt(&downloads, &jobs, settings.library_root()?, Some(&folder), request)
}

/// Opens the user's packs folder in the file manager.
#[tauri::command]
pub fn open_packs_folder(app: AppHandle) -> Result<(), String> {
    tauri_plugin_opener::open_path(packs_folder(&app)?, None::<&str>).map_err(|e| format!("Can't open the folder: {e}"))
}

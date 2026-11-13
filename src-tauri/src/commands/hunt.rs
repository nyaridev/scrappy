//! Hunt commands: the source packs (saving an edited one, creating and removing the user's own), starting a hunt and
//! reading its progress, and the user's packs folder.

use std::path::PathBuf;
use std::sync::Arc;

use scrappy_core::hunt::{self, HuntRequest, HuntStarted, HuntStatus, Hunts, Pack, Packs};
use scrappy_core::jobs::{JobId, Jobs};
use scrappy_core::media::Downloads;
use scrappy_core::settings::Store;
use tauri::{AppHandle, Manager, State};

/// The user's packs folder, made if it's missing.
fn packs_folder(app: &AppHandle) -> Result<PathBuf, String> {
    let config = app.path().app_config_dir().map_err(|e| format!("Can't find the config folder: {e}"))?;
    hunt::packs_folder(&config)
}

/// The built-in packs and the user's.
#[tauri::command(async)]
pub fn hunt_packs(app: AppHandle) -> Packs {
    hunt::packs(packs_folder(&app).ok().as_deref())
}

/// Saves an edited pack as the user's; returns every pack, read again.
#[tauri::command(async)]
pub fn save_pack(app: AppHandle, pack: Pack) -> Result<Packs, String> {
    hunt::save_pack(&packs_folder(&app)?, pack)
}

/// Saves a new pack of the user's under an id core makes from its name; returns the id (ADR-0028).
#[tauri::command(async)]
pub fn create_pack(app: AppHandle, pack: Pack) -> Result<String, String> {
    hunt::create_pack(&packs_folder(&app)?, pack)
}

/// Deletes the user's own pack `id`; returns every pack, read again. Built-in packs can't be removed (ADR-0028).
#[tauri::command(async)]
pub fn remove_pack(app: AppHandle, id: String) -> Result<Packs, String> {
    hunt::remove_pack(&packs_folder(&app)?, &id)
}

/// Starts a hunt; progress arrives as job events with the returned id, and `hunt_status` has each source's.
#[tauri::command]
pub async fn start_hunt(
    app: AppHandle,
    downloads: State<'_, Downloads>,
    jobs: State<'_, Arc<Jobs>>,
    hunts: State<'_, Hunts>,
    settings: State<'_, Store>,
    request: HuntRequest,
) -> Result<HuntStarted, String> {
    let folder = packs_folder(&app)?;
    hunt::hunt(&downloads, &jobs, &hunts, settings.library_root()?, Some(&folder), request)
}

/// The hunt `id` as it is now (its sources, and its duplicates removed); none once a newer hunt started.
#[tauri::command]
pub fn hunt_status(hunts: State<Hunts>, id: JobId) -> Option<HuntStatus> {
    hunts.status(id)
}

/// Opens the user's packs folder in the file manager.
#[tauri::command(async)]
pub fn open_packs_folder(app: AppHandle) -> Result<(), String> {
    tauri_plugin_opener::open_path(packs_folder(&app)?, None::<&str>).map_err(|e| format!("Can't open the folder: {e}"))
}

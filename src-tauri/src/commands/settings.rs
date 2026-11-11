//! Settings commands: reading the settings, saving the look and how downloads run, and the remembered sign-ins. The
//! library folder changes only through setup.

use scrappy_core::media::Downloads;
use scrappy_core::settings::{DownloadLimits, Look, Settings, Store};
use scrappy_core::vault::{RememberedSite, Vault};
use tauri::State;

/// The current settings.
#[tauri::command]
pub fn get_settings(settings: State<Store>) -> Settings {
    settings.get()
}

/// Saves a new look and returns the settings.
#[tauri::command(async)]
pub fn set_look(settings: State<Store>, look: Look) -> Result<Settings, String> {
    settings.set_look(look)
}

/// Saves how downloads run, applies it to the queue, and returns the settings.
#[tauri::command]
pub async fn set_download_limits(
    settings: State<'_, Store>,
    downloads: State<'_, Downloads>,
    limits: DownloadLimits,
) -> Result<Settings, String> {
    let saved = settings.set_download_limits(limits)?;
    downloads.set_limits(&saved.downloads);
    Ok(saved)
}

/// The sites whose sign-in is remembered.
#[tauri::command(async)]
pub fn signed_in_sites(vault: State<Vault>) -> Result<Vec<RememberedSite>, String> {
    vault.sites()
}

/// Forgets a site's sign-in.
#[tauri::command(async)]
pub fn forget_site(vault: State<Vault>, site: String) -> Result<(), String> {
    vault.forget(&site)
}

//! Settings commands: reading the settings, saving the look, and the remembered sign-ins. The library folder changes
//! only through setup.

use scrappy_core::settings::{Look, Settings, Store};
use scrappy_core::vault::{RememberedSite, Vault};
use tauri::State;

/// The current settings.
#[tauri::command]
pub fn get_settings(settings: State<Store>) -> Settings {
    settings.get()
}

/// Saves a new look and returns the settings.
#[tauri::command]
pub fn set_look(settings: State<Store>, look: Look) -> Result<Settings, String> {
    settings.set_look(look)
}

/// The sites whose sign-in is remembered.
#[tauri::command]
pub fn signed_in_sites(vault: State<Vault>) -> Result<Vec<RememberedSite>, String> {
    vault.sites()
}

/// Forgets a site's sign-in.
#[tauri::command]
pub fn forget_site(vault: State<Vault>, site: String) -> Result<(), String> {
    vault.forget(&site)
}

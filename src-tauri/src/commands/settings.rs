//! Settings commands: reading the settings, saving the look, how downloads run and the preferences, a download name's
//! preview, and the remembered sign-ins. The library folder changes only through setup.

use scrappy_core::media::Downloads;
use scrappy_core::settings::{DownloadLimits, Look, Player, Preferences, Settings, Store};
use scrappy_core::vault::{RememberedSite, Vault};
use tauri::{AppHandle, State};

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

/// Saves which Hunt packs are left out of its Pack list, and returns the settings (ADR-0028).
#[tauri::command(async)]
pub fn set_hidden_packs(settings: State<Store>, ids: Vec<String>) -> Result<Settings, String> {
    settings.set_hidden_packs(ids)
}

/// Saves the Gallery player's volume and mute, and returns the settings.
#[tauri::command(async)]
pub fn set_player(settings: State<Store>, player: Player) -> Result<Settings, String> {
    settings.set_player(player)
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

/// Saves the preferences, hands them to the tools and requests, turns starting with the system on or off, and returns
/// the settings (ADR-0034, ADR-0036).
#[tauri::command(async)]
pub fn set_preferences(app: AppHandle, settings: State<Store>, preferences: Preferences) -> Result<Settings, String> {
    let saved = settings.set_preferences(preferences)?;
    scrappy_core::app::apply(&saved);
    crate::autostart::sync(&app, saved.preferences.start_with_system)?;
    Ok(saved)
}

/// Where a single video goes with the name `template`, shown with a made-up video.
#[tauri::command]
pub fn name_example(template: String) -> String {
    scrappy_core::media::example_name(&template)
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

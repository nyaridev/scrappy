//! Autostart: starting Scrappy, minimized, when the user signs in, through Tauri's autostart plugin (ADR-0036). The
//! preference wins: saving it turns the system's entry on or off, and a release build puts back an entry an upgrade
//! removed. The Windows installer's box sets the preference once, at the next start (ADR-0041). Debug builds leave
//! the system and the installer's choice alone, so a dev run never points it at `target/debug`.

use scrappy_core::settings::{Preferences, Store};
use tauri::plugin::TauriPlugin;
use tauri::{AppHandle, Runtime};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};

/// The argument the system starts Scrappy with: open minimized.
pub const MINIMIZED: &str = "--minimized";

/// The plugin, starting Scrappy with `MINIMIZED`.
pub fn plugin<R: Runtime>() -> TauriPlugin<R> {
    tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, Some(vec![MINIMIZED]))
}

/// This run was started by the system at sign-in.
pub fn started_minimized() -> bool {
    std::env::args().any(|arg| arg == MINIMIZED)
}

/// Makes the system's entry match `on`.
pub fn sync<R: Runtime>(app: &AppHandle<R>, on: bool) -> Result<(), String> {
    if cfg!(debug_assertions) {
        return Ok(());
    }
    let launcher = app.autolaunch();
    let result = match (on, launcher.is_enabled().unwrap_or(!on)) {
        (true, false) => launcher.enable(),
        (false, true) => launcher.disable(),
        _ => Ok(()),
    };
    let what = if cfg!(windows) { "start with Windows" } else { "open at login" };
    result.map_err(|error| format!("Couldn't change {what}: {error}"))
}

/// At start: takes the installer's box into the preference when an install left one, and makes the entry match it;
/// otherwise turns the entry back on when the preference says so but the system lost it, and never turns it off.
pub fn restore<R: Runtime>(app: &AppHandle<R>, store: &Store) {
    if cfg!(debug_assertions) {
        return;
    }
    let preferences = store.get().preferences;
    let (on, chosen) = match scrappy_core::app::take_installer_choice() {
        Some(on) => (on, true),
        None => (preferences.start_with_system, false),
    };
    if chosen && let Err(error) = store.set_preferences(Preferences { start_with_system: on, ..preferences }) {
        tracing::warn!("{error}");
    }
    if (on || chosen) && let Err(error) = sync(app, on) {
        tracing::warn!("{error}");
    }
}

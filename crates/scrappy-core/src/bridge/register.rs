//! Register: connecting the browsers to scrappy-host for the app that's running (ADR-0014). Windows: a host manifest
//! in the app's data folder and a per-user registry key per browser. macOS and Linux: a manifest in each installed
//! browser's own folder. An AppImage's host is copied out first, since its files vanish when it closes.

// ── Imports ───────────────────────────────
use std::path::{Path, PathBuf};

use serde::Serialize;
use ts_rs::TS;

// ── Constants ─────────────────────────────
const NAME: &str = "com.nyaridev.scrappy";
/// The ID Chrome gives the extension, from the key in `extension/wxt.config.ts`.
const CHROME_ID: &str = "agmacpepkdohlgbkbgnafcmjggjmeohj";
const FIREFOX_ID: &str = "scrappy@nyaridev";
const DESCRIPTION: &str = "Relays the Scrappy extension's messages to the Scrappy app.";
#[cfg(windows)]
const HOST: &str = "scrappy-host.exe";
#[cfg(not(windows))]
const HOST: &str = "scrappy-host";
/// The folder in the app's data folder for Windows manifests and an AppImage's host copy.
const FOLDER: &str = "native-host";
/// Beside a copied host: the app it starts (`scrappy-host`'s `launch.rs` reads it).
const APP_PATH: &str = "app-path";

/// Each browser: its name, whether it's Firefox, its registry key under HKCU, and its manifest folders under the home
/// on macOS and Linux.
const BROWSERS: &[(&str, bool, &str, &str, &str)] = &[
    ("Chrome", false, r"Software\Google\Chrome", "Library/Application Support/Google/Chrome/NativeMessagingHosts", ".config/google-chrome/NativeMessagingHosts"),
    ("Brave", false, r"Software\BraveSoftware\Brave-Browser", "Library/Application Support/BraveSoftware/Brave-Browser/NativeMessagingHosts", ".config/BraveSoftware/Brave-Browser/NativeMessagingHosts"),
    ("Edge", false, r"Software\Microsoft\Edge", "Library/Application Support/Microsoft Edge/NativeMessagingHosts", ".config/microsoft-edge/NativeMessagingHosts"),
    ("Chromium", false, r"Software\Chromium", "Library/Application Support/Chromium/NativeMessagingHosts", ".config/chromium/NativeMessagingHosts"),
    ("Firefox", true, r"Software\Mozilla", "Library/Application Support/Mozilla/NativeMessagingHosts", ".mozilla/native-messaging-hosts"),
];

// ── Types ─────────────────────────────────
/// Which browsers reach this app through the extension.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct HostStatus {
    /// The browsers connected to this app's host.
    pub connected: Vec<String>,
    /// False when no scrappy-host sits beside the app (a development build without one).
    pub available: bool,
}

// ── Private helpers ───────────────────────
fn manifest(host: &Path, firefox: bool) -> String {
    let mut manifest = serde_json::json!({ "name": NAME, "description": DESCRIPTION, "path": host, "type": "stdio" });
    if firefox {
        manifest["allowed_extensions"] = serde_json::json!([FIREFOX_ID]);
    } else {
        manifest["allowed_origins"] = serde_json::json!([format!("chrome-extension://{CHROME_ID}/")]);
    }
    serde_json::to_string_pretty(&manifest).unwrap_or_default() + "\n"
}

/// Whether the manifest file at `file` points at `host`.
fn points_at(file: &Path, host: &Path) -> bool {
    let read = std::fs::read_to_string(file).ok().and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok());
    read.is_some_and(|manifest| manifest["path"].as_str() == Some(host.to_string_lossy().as_ref()))
}

/// The host the browsers start: beside the app, or an AppImage's copy in the data folder.
fn host_path(app_dir: &Path, data: &Path) -> PathBuf {
    if std::env::var_os("APPIMAGE").is_some() { data.join(FOLDER).join(HOST) } else { app_dir.join(HOST) }
}

/// Copies an AppImage's host out of the image, with the AppImage's path beside it for the host to start.
fn copy_out(beside: &Path, data: &Path) -> Result<(), String> {
    let Some(image) = std::env::var_os("APPIMAGE") else { return Ok(()) };
    let failed = |e: std::io::Error| format!("Can't copy scrappy-host out of the AppImage: {e}");
    let folder = data.join(FOLDER);
    std::fs::create_dir_all(&folder).map_err(failed)?;
    std::fs::copy(beside, folder.join(HOST)).map_err(failed)?;
    std::fs::write(folder.join(APP_PATH), image.to_string_lossy().as_bytes()).map_err(failed)
}

/// macOS and Linux: each browser's manifest file under `home`, and whether that browser is installed (its settings
/// folder exists).
#[cfg(any(not(windows), test))]
fn unix_files(home: &Path, mac: bool) -> Vec<(&'static str, bool, PathBuf, bool)> {
    BROWSERS
        .iter()
        .map(|(name, firefox, _, mac_folder, linux_folder)| {
            let folder = home.join(if mac { mac_folder } else { linux_folder });
            let installed = folder.parent().is_some_and(Path::is_dir);
            (*name, *firefox, folder.join(format!("{NAME}.json")), installed)
        })
        .collect()
}

#[cfg(any(not(windows), test))]
fn unix_connect(home: &Path, mac: bool, host: &Path) -> Result<(), String> {
    for (name, firefox, file, installed) in unix_files(home, mac) {
        if !installed {
            continue;
        }
        let folder = file.parent().unwrap_or(home);
        std::fs::create_dir_all(folder).and_then(|()| std::fs::write(&file, manifest(host, firefox))).map_err(|e| format!("Can't connect {name}: {e}"))?;
    }
    Ok(())
}

#[cfg(any(not(windows), test))]
fn unix_connected(home: &Path, mac: bool, host: &Path) -> Vec<String> {
    unix_files(home, mac).into_iter().filter(|(_, _, file, _)| points_at(file, host)).map(|(name, ..)| name.to_string()).collect()
}

#[cfg(any(not(windows), test))]
fn unix_disconnect(home: &Path, mac: bool) {
    for (_, _, file, _) in unix_files(home, mac) {
        let _ = std::fs::remove_file(file);
    }
}

#[cfg(not(windows))]
fn home() -> Result<PathBuf, String> {
    std::env::var_os("HOME").map(PathBuf::from).ok_or_else(|| "Can't find the home folder.".to_string())
}

#[cfg(windows)]
fn key(browser: &str) -> String {
    format!(r"{browser}\NativeMessagingHosts\{NAME}")
}

#[cfg(windows)]
fn platform_connect(host: &Path, data: &Path) -> Result<(), String> {
    use winreg::RegKey;
    use winreg::enums::HKEY_CURRENT_USER;
    let folder = data.join(FOLDER);
    let failed = |e: std::io::Error| format!("Can't connect the browsers: {e}");
    std::fs::create_dir_all(&folder).map_err(failed)?;
    let user = RegKey::predef(HKEY_CURRENT_USER);
    for (_, firefox, browser, ..) in BROWSERS {
        let file = folder.join(if *firefox { "firefox.json" } else { "chromium.json" });
        std::fs::write(&file, manifest(host, *firefox)).map_err(failed)?;
        let (entry, _) = user.create_subkey(key(browser)).map_err(failed)?;
        entry.set_value("", &file.to_string_lossy().as_ref()).map_err(failed)?;
    }
    Ok(())
}

#[cfg(windows)]
fn platform_connected(host: &Path) -> Vec<String> {
    use winreg::RegKey;
    use winreg::enums::HKEY_CURRENT_USER;
    let user = RegKey::predef(HKEY_CURRENT_USER);
    BROWSERS
        .iter()
        .filter(|(_, _, browser, ..)| {
            let file: Option<String> = user.open_subkey(key(browser)).ok().and_then(|entry| entry.get_value("").ok());
            file.is_some_and(|file| points_at(Path::new(&file), host))
        })
        .map(|(name, ..)| name.to_string())
        .collect()
}

#[cfg(windows)]
fn platform_disconnect() {
    use winreg::RegKey;
    use winreg::enums::HKEY_CURRENT_USER;
    let user = RegKey::predef(HKEY_CURRENT_USER);
    for (_, _, browser, ..) in BROWSERS {
        let _ = user.delete_subkey(key(browser));
    }
}

#[cfg(not(windows))]
fn platform_connect(host: &Path, _: &Path) -> Result<(), String> {
    unix_connect(&home()?, cfg!(target_os = "macos"), host)
}

#[cfg(not(windows))]
fn platform_connected(host: &Path) -> Vec<String> {
    home().map(|home| unix_connected(&home, cfg!(target_os = "macos"), host)).unwrap_or_default()
}

#[cfg(not(windows))]
fn platform_disconnect() {
    if let Ok(home) = home() {
        unix_disconnect(&home, cfg!(target_os = "macos"));
    }
}

// ── Public API ────────────────────────────
/// Which browsers start this app's host. `app_dir` is the running app's folder, `data` its data folder.
pub fn host_status(app_dir: &Path, data: &Path) -> HostStatus {
    HostStatus { connected: platform_connected(&host_path(app_dir, data)), available: app_dir.join(HOST).is_file() }
}

/// Connects every installed browser to this app's host.
pub fn connect(app_dir: &Path, data: &Path) -> Result<HostStatus, String> {
    let beside = app_dir.join(HOST);
    if !beside.is_file() {
        return Err("scrappy-host isn't next to the app.".into());
    }
    copy_out(&beside, data)?;
    platform_connect(&host_path(app_dir, data), data)?;
    let status = host_status(app_dir, data);
    if status.connected.is_empty() {
        return Err("No Chrome, Brave, Edge, Chromium or Firefox found.".into());
    }
    Ok(status)
}

/// Disconnects every browser from Scrappy's host.
pub fn disconnect(app_dir: &Path, data: &Path) -> HostStatus {
    platform_disconnect();
    host_status(app_dir, data)
}

#[cfg(test)]
mod tests {
    use super::{manifest, unix_connect, unix_connected, unix_disconnect};
    use std::path::Path;

    #[test]
    fn writes_manifests_for_installed_browsers() {
        let home = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(home.path().join(".config/google-chrome")).unwrap();
        std::fs::create_dir_all(home.path().join(".mozilla")).unwrap();
        let host = Path::new("/opt/scrappy/scrappy-host");
        unix_connect(home.path(), false, host).unwrap();
        assert_eq!(unix_connected(home.path(), false, host), ["Chrome", "Firefox"]);
        assert!(unix_connected(home.path(), false, Path::new("/elsewhere/scrappy-host")).is_empty());
        let chrome = std::fs::read_to_string(home.path().join(".config/google-chrome/NativeMessagingHosts/com.nyaridev.scrappy.json")).unwrap();
        assert!(chrome.contains("chrome-extension://agmacpepkdohlgbkbgnafcmjggjmeohj/") && !chrome.contains("allowed_extensions"));
        assert!(!home.path().join(".config/chromium/NativeMessagingHosts").exists(), "Chromium isn't installed");
        unix_disconnect(home.path(), false);
        assert!(unix_connected(home.path(), false, host).is_empty());
        assert!(manifest(host, true).contains("scrappy@nyaridev"));
    }
}

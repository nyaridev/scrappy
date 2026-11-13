//! App: facts about the running build that the window shows, which frame the window gets (ADR-0011), and the
//! choice the Windows installer hands over (ADR-0041).

// ── Imports ───────────────────────────────
use serde::Serialize;
use ts_rs::TS;

use crate::settings::Settings;
use crate::{dedupe, media, net};

// ── Constants ─────────────────────────────
/// The User-Agent for Scrappy's own requests: it names the app and nothing about the user (ADR-0005).
pub const USER_AGENT: &str = concat!("Scrappy/", env!("CARGO_PKG_VERSION"));
/// Where the Windows installer leaves its "Start Scrappy with Windows" box: `src-tauri/windows/hooks.nsh` writes it.
#[cfg(windows)]
const INSTALLER_CHOICE: (&str, &str) = (r"Software\com.nyaridev.scrappy", "StartWithSystem");

// ── Types ─────────────────────────────────
/// The running version and platform, shown in Settings → About.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AppInfo {
    pub version: String,
    pub os: String,
    pub frame: WindowFrame,
}

/// How the window is framed, so the header knows what to draw.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum WindowFrame {
    /// No system frame: the header is the title bar and draws its own window buttons (Windows, Linux).
    Custom,
    /// The system's window buttons over the header's left end; the header is the title bar (macOS).
    Overlay,
    /// The system's own title bar above a plain header.
    Native,
}

/// What Scrappy's own updater is doing (ADR-0037).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum UpdateStep {
    Checking,
    Downloading,
    Installing,
}

/// Scrappy's own version and what GitHub has (ADR-0037); the shell fills it in and the window shows it.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct UpdateStatus {
    pub current: String,
    /// A newer version, once a check found one.
    pub latest: Option<String>,
    pub step: Option<UpdateStep>,
    /// How much of the update has downloaded, 0 to 1, while it does.
    pub progress: Option<f64>,
    /// Why the last check or install failed.
    pub error: Option<String>,
    /// This build can update itself: a release build. A debug build never checks.
    pub can_update: bool,
    /// When the last check finished, in Unix seconds.
    #[ts(type = "number | null")]
    pub checked_at: Option<i64>,
}

impl UpdateStatus {
    /// Nothing checked yet.
    pub fn new(can_update: bool) -> Self {
        Self {
            current: env!("CARGO_PKG_VERSION").into(),
            latest: None,
            step: None,
            progress: None,
            error: None,
            can_update,
            checked_at: None,
        }
    }
}

// ── Private helpers ───────────────────────
fn frame_for(os: &str, native_frame: bool) -> WindowFrame {
    match (native_frame, os) {
        (true, _) => WindowFrame::Native,
        (false, "macos") => WindowFrame::Overlay,
        _ => WindowFrame::Custom,
    }
}

// ── Public API ────────────────────────────
/// Describes this build and the frame its window got at start.
pub fn info(frame: WindowFrame) -> AppInfo {
    AppInfo { version: env!("CARGO_PKG_VERSION").to_string(), os: std::env::consts::OS.to_string(), frame }
}

/// Hands the preferences the tools and requests read to the modules that keep them (ADR-0034): the proxy, what
/// yt-dlp saves and where a single download goes, and how strictly copies are removed on their own. Called at start
/// and after each save.
pub fn apply(settings: &Settings) {
    let preferences = &settings.preferences;
    net::set_proxy(Some(preferences.proxy.clone()).filter(|proxy| !proxy.is_empty()));
    media::set_media_options(preferences.media.clone());
    dedupe::set_auto_strictness(preferences.duplicates);
}

/// The installer's "Start Scrappy with Windows" box, when an install left it since the last start: read once, then
/// removed (ADR-0041). Always `None` on macOS and Linux.
pub fn take_installer_choice() -> Option<bool> {
    #[cfg(windows)]
    {
        use winreg::RegKey;
        use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_WRITE};
        let user = RegKey::predef(HKEY_CURRENT_USER);
        let key = user.open_subkey_with_flags(INSTALLER_CHOICE.0, KEY_READ | KEY_WRITE).ok()?;
        let chosen: u32 = key.get_value(INSTALLER_CHOICE.1).ok()?;
        let _ = key.delete_value(INSTALLER_CHOICE.1);
        Some(chosen != 0)
    }
    #[cfg(not(windows))]
    None
}

/// The frame for this platform, or the system's own when the look's `native_frame` is on.
pub fn window_frame(native_frame: bool) -> WindowFrame {
    frame_for(std::env::consts::OS, native_frame)
}

#[cfg(test)]
mod tests {
    use super::{WindowFrame, frame_for};

    #[test]
    fn picks_the_frame_per_platform() {
        assert_eq!(frame_for("windows", false), WindowFrame::Custom);
        assert_eq!(frame_for("linux", false), WindowFrame::Custom);
        assert_eq!(frame_for("macos", false), WindowFrame::Overlay);
    }

    #[test]
    fn native_frame_wins_everywhere() {
        for os in ["windows", "linux", "macos"] {
            assert_eq!(frame_for(os, true), WindowFrame::Native);
        }
    }
}

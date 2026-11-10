//! Browser: the installed Chrome, Brave or Edge that Grab's deep scan drives (ADR-0007). Scrappy never downloads one
//! and never opens the user's own profile.

// ── Imports ───────────────────────────────
use std::path::PathBuf;

use super::toolbox::on_path;

// ── Constants ─────────────────────────────
/// Windows installs: the folder's environment variable, then the program inside it.
const WINDOWS: &[(&str, &str)] = &[
    ("ProgramFiles", "Google/Chrome/Application/chrome.exe"),
    ("ProgramFiles(x86)", "Google/Chrome/Application/chrome.exe"),
    ("LOCALAPPDATA", "Google/Chrome/Application/chrome.exe"),
    ("ProgramFiles", "BraveSoftware/Brave-Browser/Application/brave.exe"),
    ("ProgramFiles(x86)", "BraveSoftware/Brave-Browser/Application/brave.exe"),
    ("LOCALAPPDATA", "BraveSoftware/Brave-Browser/Application/brave.exe"),
    ("ProgramFiles(x86)", "Microsoft/Edge/Application/msedge.exe"),
    ("ProgramFiles", "Microsoft/Edge/Application/msedge.exe"),
];
const MACOS: &[&str] = &[
    "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
    "/Applications/Brave Browser.app/Contents/MacOS/Brave Browser",
    "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
    "/Applications/Chromium.app/Contents/MacOS/Chromium",
];
/// Linux (and anything else) finds them on `PATH`.
const ON_PATH: &[&str] = &[
    "google-chrome",
    "google-chrome-stable",
    "chromium",
    "chromium-browser",
    "brave-browser",
    "microsoft-edge",
    "microsoft-edge-stable",
];

// ── Public API ────────────────────────────
/// The first installed Chrome, Brave or Edge, in that order.
pub fn browser() -> Option<PathBuf> {
    let windows = WINDOWS
        .iter()
        .filter_map(|(var, program)| std::env::var_os(var).map(|dir| PathBuf::from(dir).join(program)));
    let macos = MACOS.iter().map(PathBuf::from);
    windows.chain(macos).find(|path| path.is_file()).or_else(|| ON_PATH.iter().find_map(|name| on_path(name)))
}

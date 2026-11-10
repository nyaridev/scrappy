//! App: facts about the running build that the window shows.

// ── Imports ───────────────────────────────
use serde::Serialize;
use ts_rs::TS;

// ── Types ─────────────────────────────────
/// The running version and platform, shown in Settings → About.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AppInfo {
    pub version: String,
    pub os: String,
}

// ── Public API ────────────────────────────
/// Describes this build.
pub fn info() -> AppInfo {
    AppInfo {
        version: env!("CARGO_PKG_VERSION").to_string(),
        os: std::env::consts::OS.to_string(),
    }
}

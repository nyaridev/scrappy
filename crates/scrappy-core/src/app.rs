//! App: facts about the running build that the window shows, and which frame the window gets (ADR-0011).

// ── Imports ───────────────────────────────
use serde::Serialize;
use ts_rs::TS;

// ── Constants ─────────────────────────────
/// The User-Agent for Scrappy's own requests: it names the app and nothing about the user (ADR-0005).
pub const USER_AGENT: &str = concat!("Scrappy/", env!("CARGO_PKG_VERSION"));

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

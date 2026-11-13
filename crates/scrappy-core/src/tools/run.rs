//! Run: starting a tool's program with no console window on Windows, reading its version, and letting yt-dlp update
//! itself (ADR-0002).

// ── Imports ───────────────────────────────
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use tokio::process::Command;

use super::manifest::Tool;

// ── Constants ─────────────────────────────
/// Windows' `CREATE_NO_WINDOW`: no console window flashes up for each run.
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const VERSION_TIMEOUT: Duration = Duration::from_secs(15);
const UPDATE_TIMEOUT: Duration = Duration::from_secs(300);

// ── Private helpers ───────────────────────
/// The version in a program's `--version` output: yt-dlp and gallery-dl print it alone, Deno as `deno 2.9.8 (…)`,
/// FFmpeg as `ffmpeg version n9.0.2-39-g… Copyright …`.
pub(super) fn parse_version(tool: Tool, output: &str) -> Option<String> {
    let line = output.lines().next()?.trim();
    let version = match tool {
        Tool::Deno => line.split_whitespace().nth(1),
        Tool::Ffmpeg => line.split_whitespace().nth(2),
        Tool::YtDlp | Tool::GalleryDl => Some(line),
    }?;
    (!version.is_empty()).then(|| version.to_string())
}

/// The leading numbers of a version: `n9.0.2-39-g…` → `[9, 0, 2]`, `1.33.0-dev:…` → `[1, 33, 0]`. `None` when it
/// doesn't start with one (FFmpeg's `N-128227-g…` master builds).
fn numbers(version: &str) -> Option<Vec<u64>> {
    let digits: String = version
        .trim()
        .trim_start_matches(['v', 'n'])
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    let parts: Option<Vec<u64>> = digits.split('.').filter(|part| !part.is_empty()).map(|p| p.parse().ok()).collect();
    parts.filter(|parts| !parts.is_empty())
}

/// Whether `version` is older than `pinned`; `None` when either can't be read.
pub(super) fn older_than(version: &str, pinned: &str) -> Option<bool> {
    Some(numbers(version)? < numbers(pinned)?)
}

// ── Public API ────────────────────────────
/// A command for `program` that opens no window and gets no input. Every run of a tool starts here.
pub fn command(program: &Path) -> Command {
    let mut command = Command::new(program);
    command.stdin(Stdio::null()).kill_on_drop(true);
    // yt-dlp and gallery-dl are Python programs: UTF-8 for what they read and write. Their printing ignores this
    // on Windows; `media::engine` gives each its own flag for that.
    command.env("PYTHONUTF8", "1").env("PYTHONIOENCODING", "utf-8");
    #[cfg(windows)]
    command.creation_flags(CREATE_NO_WINDOW);
    command
}

/// The version `program` reports, or `None` when it doesn't run.
pub(super) async fn version(tool: Tool, program: &Path) -> Option<String> {
    let flag = if tool == Tool::Ffmpeg { "-version" } else { "--version" };
    let output = tokio::time::timeout(VERSION_TIMEOUT, command(program).arg(flag).output()).await.ok()?.ok()?;
    parse_version(tool, &String::from_utf8_lossy(&output.stdout))
}

/// Lets yt-dlp update itself to the nightly build. Its updater checks the update's SHA-256 (ADR-0012). Only logs:
/// a failed update leaves the working version in place.
pub(super) async fn update_yt_dlp(program: &Path) {
    let run = command(program).args(["--update-to", "nightly"]).output();
    match tokio::time::timeout(UPDATE_TIMEOUT, run).await {
        Ok(Ok(output)) if output.status.success() => tracing::info!("yt-dlp update check done"),
        Ok(Ok(output)) => tracing::warn!("yt-dlp update failed with {}", output.status),
        Ok(Err(error)) => tracing::warn!("yt-dlp update didn't start: {error}"),
        Err(_) => tracing::warn!("yt-dlp update timed out"),
    }
}

#[cfg(test)]
mod tests {
    use super::{Tool, older_than, parse_version};

    #[test]
    fn compares_versions_by_their_numbers() {
        assert_eq!(older_than("2024.08.06", "2026.10.26"), Some(true));
        assert_eq!(older_than("2026.11.05.233012", "2026.10.26"), Some(false));
        assert_eq!(older_than("2026.10.26", "2026.10.26"), Some(false));
        assert_eq!(older_than("n9.0.2-39-g7adfbe40dc", "9.0.2"), Some(false));
        assert_eq!(older_than("7.1-full_build-www.gyan.dev", "9.0.2"), Some(true));
        assert_eq!(older_than("1.33.0-dev:2026.11.10", "1.33.0"), Some(false));
        assert_eq!(older_than("v2.1.0", "2.9.8"), Some(true));
        assert_eq!(older_than("N-128227-g4811625b5f", "9.0.2"), None);
    }

    #[test]
    fn reads_each_tools_version_line() {
        assert_eq!(parse_version(Tool::YtDlp, "2026.10.26\n").as_deref(), Some("2026.10.26"));
        assert_eq!(parse_version(Tool::GalleryDl, "1.32.19-dev\n").as_deref(), Some("1.32.19-dev"));
        let deno = "deno 2.9.8 (stable, release, x86_64-pc-windows-msvc)\nv8 14.2\ntypescript 5.9";
        assert_eq!(parse_version(Tool::Deno, deno).as_deref(), Some("2.9.8"));
        let ffmpeg = "ffmpeg version n9.0.2-39-g7adfbe40dc Copyright (c) 2000-2026 the FFmpeg developers\n";
        assert_eq!(parse_version(Tool::Ffmpeg, ffmpeg).as_deref(), Some("n9.0.2-39-g7adfbe40dc"));
        assert_eq!(parse_version(Tool::Deno, ""), None);
    }
}

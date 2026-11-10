//! Manifest: the tools Scrappy manages and the pinned download for each one, per system, with the SHA-256 GitHub
//! publishes for the file (ADR-0012). To update a pin, change its entry here in one commit.

// ── Imports ───────────────────────────────
use serde::{Deserialize, Serialize};
use ts_rs::TS;

// ── Types ─────────────────────────────────
/// A tool Scrappy can download and run. FFmpeg brings ffprobe with it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum Tool {
    YtDlp,
    Deno,
    GalleryDl,
    Ffmpeg,
}

/// How the downloaded file holds the tool's programs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Archive {
    /// The file is the program.
    Plain,
    Zip,
    TarXz,
}

/// One pinned download.
pub(super) struct Download {
    pub tool: Tool,
    /// `std::env::consts::OS`.
    pub os: &'static str,
    /// `std::env::consts::ARCH`, or `None` for any (universal macOS builds).
    pub arch: Option<&'static str>,
    pub url: &'static str,
    pub sha256: &'static str,
    pub archive: Archive,
}

// ── Constants ─────────────────────────────
// yt-dlp 2026.10.26, Deno 2.9.8, gallery-dl build 2026.11.10, FFmpeg 9.0.2 (BtbN month-end build 2026-10-31).
// Each source's release is written once, in its macro.
macro_rules! yt_dlp {
    ($file:literal) => {
        concat!("https://github.com/yt-dlp/yt-dlp/releases/download/2026.10.26/", $file)
    };
}
macro_rules! deno {
    ($file:literal) => {
        concat!("https://github.com/denoland/deno/releases/download/v2.9.8/", $file)
    };
}
macro_rules! gallery_dl {
    ($file:literal) => {
        concat!("https://github.com/gdl-org/builds/releases/download/2026.11.10/", $file)
    };
}
macro_rules! ffmpeg {
    ($file:literal) => {
        concat!("https://github.com/BtbN/FFmpeg-Builds/releases/download/autobuild-2026-10-31-12-59/", $file)
    };
}

#[rustfmt::skip]
const DOWNLOADS: &[Download] = &[
    // Windows
    Download { tool: Tool::YtDlp, os: "windows", arch: Some("x86_64"), archive: Archive::Plain,
        url: yt_dlp!("yt-dlp.exe"),
        sha256: "f5389d3698ecde4ef06f4e6b1ce0294fe9ac1396228c8990b8d3416d6d177a7d" },
    Download { tool: Tool::Deno, os: "windows", arch: Some("x86_64"), archive: Archive::Zip,
        url: deno!("deno-x86_64-pc-windows-msvc.zip"),
        sha256: "cba84b26e0df5ae7f74fc66b86a95b866872a86292ebd84fa1e959927edd93bf" },
    Download { tool: Tool::GalleryDl, os: "windows", arch: Some("x86_64"), archive: Archive::Plain,
        url: gallery_dl!("gallery-dl_windows.exe"),
        sha256: "80cf3339ff0e23f781cf81b14d04a1fb507cb7d3a0030c90c3383f07521129c4" },
    Download { tool: Tool::Ffmpeg, os: "windows", arch: Some("x86_64"), archive: Archive::Zip,
        url: ffmpeg!("ffmpeg-n9.0.2-39-g7adfbe40dc-win64-gpl-9.0.zip"),
        sha256: "693f3e224bc184b40524a62ca47526cfebaf2504414021b92fe75f677ad868b9" },
    // Linux
    Download { tool: Tool::YtDlp, os: "linux", arch: Some("x86_64"), archive: Archive::Plain,
        url: yt_dlp!("yt-dlp_linux"),
        sha256: "767ba9dce7052aa196f8b12652f433d76de128ba168966fa84b6394d5e6c6795" },
    Download { tool: Tool::Deno, os: "linux", arch: Some("x86_64"), archive: Archive::Zip,
        url: deno!("deno-x86_64-unknown-linux-gnu.zip"),
        sha256: "e890975bc2549807bbcc0b584ffafda72cc7c93349dd99c80483dd784f6c4609" },
    Download { tool: Tool::GalleryDl, os: "linux", arch: Some("x86_64"), archive: Archive::Plain,
        url: gallery_dl!("gallery-dl_linux"),
        sha256: "28548660805f5c074c7cff191c1b8122e98f502753b70fce2cfb2bde1bd30d56" },
    Download { tool: Tool::Ffmpeg, os: "linux", arch: Some("x86_64"), archive: Archive::TarXz,
        url: ffmpeg!("ffmpeg-n9.0.2-39-g7adfbe40dc-linux64-gpl-9.0.tar.xz"),
        sha256: "73e29a56974ae5a651133a04584c7483b21afe941e17dc91d79814a758b608d3" },
    // macOS (no FFmpeg download: it comes from PATH, ADR-0012)
    Download { tool: Tool::YtDlp, os: "macos", arch: None, archive: Archive::Plain,
        url: yt_dlp!("yt-dlp_macos"),
        sha256: "0f922c15ccc0b83eefa1c8bc9104e434318c4bf16e408e1ee93b7745a6efe0aa" },
    Download { tool: Tool::Deno, os: "macos", arch: Some("aarch64"), archive: Archive::Zip,
        url: deno!("deno-aarch64-apple-darwin.zip"),
        sha256: "9f0bf660393da2eaff044cdc7444cb08c73f235849b61d224d825a194ce7c98e" },
    Download { tool: Tool::Deno, os: "macos", arch: Some("x86_64"), archive: Archive::Zip,
        url: deno!("deno-x86_64-apple-darwin.zip"),
        sha256: "4b2f5edd2639b7470f034238b321d7002d8dfb4726d8558f52c17afac370eb2c" },
    Download { tool: Tool::GalleryDl, os: "macos", arch: None, archive: Archive::Plain,
        url: gallery_dl!("gallery-dl_macos"),
        sha256: "426db4c5d6ef47bdb712089d7f15c0c533ad5750eb1c89ce68a6b9a1af525ed3" },
];

// ── Public API ────────────────────────────
impl Tool {
    /// Every tool, in the order the window lists them.
    pub const ALL: [Tool; 4] = [Tool::YtDlp, Tool::Deno, Tool::GalleryDl, Tool::Ffmpeg];

    /// The version the pinned downloads report. A copy older than this is offered an update.
    pub fn pinned_version(self) -> &'static str {
        match self {
            Tool::YtDlp => "2026.10.26",
            Tool::Deno => "2.9.8",
            // The gdl-org build reports `1.33.0-dev:2026.11.10`.
            Tool::GalleryDl => "1.33.0",
            Tool::Ffmpeg => "9.0.2",
        }
    }

    /// The programs this tool provides; the first is the one Scrappy runs to read the version.
    pub fn programs(self) -> &'static [&'static str] {
        match self {
            Tool::YtDlp => &["yt-dlp"],
            Tool::Deno => &["deno"],
            Tool::GalleryDl => &["gallery-dl"],
            Tool::Ffmpeg => &["ffmpeg", "ffprobe"],
        }
    }
}

/// The pinned download of `tool` for this system, if there is one.
pub(super) fn download_for(tool: Tool) -> Option<&'static Download> {
    use std::env::consts::{ARCH, OS};
    DOWNLOADS.iter().find(|d| d.tool == tool && d.os == OS && d.arch.is_none_or(|arch| arch == ARCH))
}

#[cfg(test)]
mod tests {
    use super::{DOWNLOADS, Tool, download_for};

    #[test]
    fn every_pin_is_a_github_https_url_with_a_sha256() {
        for download in DOWNLOADS {
            assert!(download.url.starts_with("https://github.com/"), "{}", download.url);
            assert_eq!(download.sha256.len(), 64, "{}", download.url);
            assert!(download.sha256.bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()));
        }
    }

    #[test]
    fn this_system_has_its_downloads() {
        if cfg!(all(target_os = "windows", target_arch = "x86_64")) {
            assert!(Tool::ALL.iter().all(|&tool| download_for(tool).is_some()));
        }
    }
}

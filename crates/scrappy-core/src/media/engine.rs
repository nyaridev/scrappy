//! Engine: the check every link goes through, the yt-dlp and gallery-dl commands every run starts from (yt-dlp with
//! Scrappy's Deno and FFmpeg passed in), running a tool line by line until it ends or is stopped, sign-in walls, and
//! tool errors turned into one readable line.

// ── Imports ───────────────────────────────
use std::process::Stdio;
use std::time::Duration;

use reqwest::Url;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

use crate::jobs::Job;
use crate::tools::{self, Tool, Toolbox};
use crate::vault::CookieFile;

// ── Constants ─────────────────────────────
/// How often a running tool checks whether its job was stopped.
const CANCEL_CHECK: Duration = Duration::from_millis(200);
/// Words in yt-dlp's errors when a site wants a signed-in visitor (all lowercase).
const LOGIN_HINTS: &[&str] = &[
    "--cookies",
    "sign in",
    "log in",
    "login",
    "logged-in",
    "age-restricted",
    "confirm your age",
    "inappropriate for some users",
    "private video",
    "this video is private",
    "members-only",
    "requires authentication",
    // gallery-dl's
    "authenticated",
    "authorizationerror",
    "cookies needed",
];

// ── Public API ────────────────────────────
/// The link, if it's http or https: nothing else reaches yt-dlp (ADR-0005).
pub(super) fn check_url(input: &str) -> Result<String, String> {
    let url = Url::parse(input.trim()).map_err(|_| "Not a link.".to_string())?;
    match url.scheme() {
        "http" | "https" => Ok(url.into()),
        _ => Err("Only http and https links work.".into()),
    }
}

/// A yt-dlp command with Deno (for YouTube) and FFmpeg (for merging) wired in.
pub(super) fn yt_dlp(toolbox: &Toolbox) -> Result<Command, String> {
    let program = toolbox.locate(Tool::YtDlp).1.ok_or("yt-dlp isn't installed. Download it in Settings.")?;
    let mut command = tools::command(&program);
    command.args(["--no-colors", "--no-warnings", "--no-playlist"]);
    if let Some(deno) = toolbox.locate(Tool::Deno).1 {
        command.arg("--js-runtimes").arg(format!("deno:{}", deno.display()));
    }
    if let Some(ffmpeg) = toolbox.locate(Tool::Ffmpeg).1 {
        command.arg("--ffmpeg-location").arg(ffmpeg);
    }
    Ok(command)
}

/// A gallery-dl command that ignores any config file on this PC, so every run is the same.
pub(super) fn gallery_dl(toolbox: &Toolbox) -> Result<Command, String> {
    let program = toolbox.locate(Tool::GalleryDl).1.ok_or("gallery-dl isn't installed. Download it in Settings.")?;
    let mut command = tools::command(&program);
    command.arg("--config-ignore");
    Ok(command)
}

/// Runs `command`, handing each line it prints to `on_line`, until it ends or `job` is stopped (then it's killed
/// and this returns "Cancelled."). Returns whether it succeeded and what it printed to stderr.
pub(super) async fn run_lines(
    mut command: Command,
    name: &str,
    job: &Job,
    mut on_line: impl FnMut(&str),
) -> Result<(bool, Vec<String>), String> {
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = command.spawn().map_err(|e| format!("{name} didn't start: {e}"))?;
    let stderr = child.stderr.take().map(|stderr| {
        tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            let mut kept = Vec::new();
            while let Ok(Some(line)) = lines.next_line().await {
                kept.push(line);
            }
            kept
        })
    });
    let mut lines = BufReader::new(child.stdout.take().ok_or_else(|| format!("{name} gave no output."))?).lines();
    let stopped = loop {
        tokio::select! {
            line = lines.next_line() => match line {
                Ok(Some(line)) => on_line(&line),
                _ => break false,
            },
            () = tokio::time::sleep(CANCEL_CHECK) => if job.cancelled() { break true },
        }
    };
    if stopped {
        let _ = child.kill().await;
        return Err("Cancelled.".into());
    }
    let status = child.wait().await.map_err(|e| format!("{name} stopped: {e}"))?;
    let errors = match stderr {
        Some(task) => task.await.unwrap_or_default(),
        None => Vec::new(),
    };
    Ok((status.success(), errors))
}

/// Passes request headers to yt-dlp. Only `bridge`'s allowed ones (Referer, Origin, User-Agent) get here: cookies
/// never go through arguments (ADR-0005).
pub(super) fn add_headers(command: &mut Command, headers: &[(String, String)]) {
    for (name, value) in headers {
        command.arg("--add-header").arg(format!("{name}:{value}"));
    }
}

/// Hands a tool the temporary cookies file, when there is one (never the cookies themselves: ADR-0005).
pub(super) fn add_cookies(command: &mut Command, file: Option<&CookieFile>) {
    if let Some(file) = file {
        command.arg("--cookies").arg(file.path());
    }
}

/// Whether an error means the site wants a signed-in visitor.
pub(super) fn needs_login(error: &str) -> bool {
    let error = error.to_lowercase();
    LOGIN_HINTS.iter().any(|hint| error.contains(hint))
}

/// The line worth showing from a tool's error output: yt-dlp's last `ERROR:` line or gallery-dl's last
/// `[site][error]` line, without their prefixes, or the last line at all.
pub(super) fn error_text(lines: &[String]) -> String {
    // gallery-dl: `[twitter][error] AuthRequired: authenticated cookies needed…`; the name before `: ` is dropped.
    let gallery = lines.iter().rev().find_map(|line| line.split_once("][error] ")).map(|(_, rest)| {
        rest.split_once(": ").filter(|(name, _)| name.chars().all(char::is_alphanumeric)).map_or(rest, |(_, why)| why)
    });
    if let Some(message) = gallery {
        return message.trim().to_string();
    }
    let error = lines.iter().rev().find_map(|line| line.strip_prefix("ERROR: "));
    let line = error.or_else(|| lines.iter().rev().map(|line| line.trim()).find(|line| !line.is_empty()));
    // yt-dlp starts most errors with `[extractor] id: `; the reason follows.
    let line = line.map(|line| match line.strip_prefix('[') {
        Some(rest) => rest.split_once(": ").map_or(line, |(_, reason)| reason),
        None => line,
    });
    line.unwrap_or("The download failed.").to_string()
}

#[cfg(test)]
mod tests {
    use super::{check_url, error_text, needs_login};

    #[test]
    fn takes_only_web_links() {
        assert_eq!(check_url(" https://youtu.be/abc ").unwrap(), "https://youtu.be/abc");
        assert_eq!(check_url("file:///C:/secret").unwrap_err(), "Only http and https links work.");
        assert_eq!(check_url("--exec calc").unwrap_err(), "Not a link.");
    }

    #[test]
    fn keeps_the_reason_of_an_error() {
        let lines = vec![
            "[youtube] Extracting URL".to_string(),
            "ERROR: [youtube] abc: Video unavailable. This video is private".to_string(),
        ];
        assert_eq!(error_text(&lines), "Video unavailable. This video is private");
        assert_eq!(error_text(&["Killed".to_string(), String::new()]), "Killed");
        assert_eq!(error_text(&[]), "The download failed.");
        let gallery = vec![
            "[twitter][info] Requesting guest token".to_string(),
            "[twitter][error] AuthRequired: authenticated cookies needed to access this timeline".to_string(),
        ];
        assert_eq!(error_text(&gallery), "authenticated cookies needed to access this timeline");
        let unsupported = vec!["[gallery-dl][error] Unsupported URL 'https://example.com/'".to_string()];
        assert_eq!(error_text(&unsupported), "Unsupported URL 'https://example.com/'");
    }

    #[test]
    fn knows_a_sign_in_wall() {
        assert!(needs_login("Sign in to confirm your age. This video may be inappropriate for some users."));
        assert!(needs_login("Use --cookies-from-browser or --cookies for the authentication."));
        assert!(needs_login("Requested content is not available, rate-limit reached or login required"));
        assert!(!needs_login("Unable to download webpage: HTTP Error 404: Not Found"));
        assert!(!needs_login("Unsupported URL: https://example.com/"));
    }
}

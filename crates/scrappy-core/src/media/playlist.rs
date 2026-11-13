//! Playlist: a playlist or channel as one job. yt-dlp saves its videos into `Videos/{site}/{channel}/{playlist}/` (or
//! `Audio/…`), skipping videos saved before (its archive in `.scrappy/`) and going on past ones that fail; each saved
//! file then gets the preset's check and encode, and a catalog row. When the listing has every video (or the window
//! left some out, ADR-0016), each video gets its own yt-dlp, side by side, one download slot each (ADR-0017);
//! otherwise yt-dlp gets the list's link and works through it in one slot.

// ── Imports ───────────────────────────────
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};

use futures_util::stream::{self, StreamExt};
use tokio::process::Command;

use super::download::{FILE_MARK, Found, Saved, collection, escape_template, finish, record_file, relative, size_of};
use super::engine;
use super::presets::Preset;
use super::probe::Info;
use crate::jobs::{Job, Queue};
use crate::library::template;
use crate::tools::Toolbox;
use crate::vault::{Cookie, CookieFile};

// ── Constants ─────────────────────────────
const VIDEO_TEMPLATE: &str = "Videos/{site}/{channel}/{playlist}";
const AUDIO_TEMPLATE: &str = "Audio/{site}/{channel}/{playlist}";
const ARCHIVE: &str = ".scrappy/yt-dlp-archive.txt";
/// yt-dlp's line for a video it skipped because the archive has it.
const SKIPPED_TEXT: &str = "has already been recorded in the archive";
/// yt-dlp's line before each video: `[download] Downloading item 3 of 40`.
const ITEM_TEXT: &str = "[download] Downloading item ";

// ── Types ─────────────────────────────────
/// Everything one playlist download needs.
pub(super) struct PlaylistOrder {
    pub root: PathBuf,
    pub url: String,
    pub preset: &'static Preset,
    pub max_height: Option<u32>,
    /// Save only the first this many videos.
    pub limit: Option<u32>,
    /// The links of the videos to save, each by its own yt-dlp; `None` to give yt-dlp the list's link.
    pub links: Option<Vec<String>>,
    /// yt-dlp's JSON from the probe: the playlist's title and channel.
    pub info_json: String,
    pub cookies: Option<Vec<Cookie>>,
    /// The app's download slots: each yt-dlp run takes one.
    pub slots: Queue,
}

/// What the yt-dlp runs saw so far.
#[derive(Default)]
struct Seen {
    saved: Vec<PathBuf>,
    skipped: u32,
    total: Option<u64>,
    /// A run failed; `errors` holds its last error output.
    failed: bool,
    errors: Vec<String>,
}

// ── Private helpers ───────────────────────
/// The first `limit` of `links`, web links only.
fn picked(links: &[String], limit: Option<u32>) -> Result<Vec<String>, String> {
    let take = limit.map_or(usize::MAX, |limit| limit as usize);
    let links: Vec<String> = links.iter().filter(|link| engine::check_url(link).is_ok()).take(take).cloned().collect();
    if links.is_empty() {
        return Err("No videos are picked.".into());
    }
    Ok(links)
}

/// The total from `[download] Downloading item 3 of 40`.
fn item_total(line: &str) -> Option<u64> {
    line.strip_prefix(ITEM_TEXT)?.split_once(" of ")?.1.trim().parse().ok()
}

/// yt-dlp saving into `folder` with the preset and the archive; the caller adds the link.
fn command(toolbox: &Toolbox, order: &PlaylistOrder, folder: &Path, cookies: Option<&CookieFile>) -> Result<Command, String> {
    let mut command = engine::yt_dlp(toolbox)?;
    engine::add_cookies(&mut command, cookies);
    command
        // `--no-quiet` keeps the "already recorded" lines that `--print` would hide; `-i` goes on past a failed video.
        .args(["--newline", "--no-mtime", "--no-simulate", "--no-quiet", "--no-progress", "-i"])
        .args(["--print", &format!("after_move:{FILE_MARK}%(filepath)s")])
        .args(["-f", &order.preset.format(order.max_height)])
        .args(order.preset.yt_dlp)
        // shortcut: runs side by side append to one archive; a line lost to a clash only means that video downloads
        // again next time. Upgrade to Rust writing the archive if that shows up.
        .arg("--download-archive")
        .arg(order.root.join(ARCHIVE))
        .arg("-o")
        .arg(format!("{}/%(title)s [%(id)s].%(ext)s", escape_template(&folder.to_string_lossy())));
    engine::add_extras(&mut command, order.preset.audio);
    Ok(command)
}

/// Reads one of yt-dlp's lines into `seen` and reports the files done.
fn read_line(line: &str, seen: &Mutex<Seen>, job: &Job) {
    let mut seen = seen.lock().unwrap_or_else(PoisonError::into_inner);
    let name = if let Some(path) = line.strip_prefix(FILE_MARK) {
        let path = PathBuf::from(path.trim());
        let name = path.file_name().map(|name| name.to_string_lossy().into_owned());
        seen.saved.push(path);
        name
    } else if line.contains(SKIPPED_TEXT) {
        seen.skipped += 1;
        None
    } else {
        seen.total = item_total(line).or(seen.total);
        return;
    };
    let done = seen.saved.len() as u64 + u64::from(seen.skipped);
    job.items(done, seen.total, name);
}

/// Runs one yt-dlp in its own slot, noting a failure in `seen`.
async fn run_one(command: Result<Command, String>, slots: &Queue, seen: &Mutex<Seen>, job: &Job) {
    let ran = match command {
        Ok(command) => match slots.slot(job).await {
            Ok(_slot) => engine::run_lines(command, "yt-dlp", job, |line| read_line(line, seen, job)).await,
            Err(error) => Err(error),
        },
        Err(error) => Err(error),
    };
    let errors = match ran {
        Ok((true, _)) => return,
        Ok((false, errors)) => errors,
        Err(error) => vec![error],
    };
    let mut seen = seen.lock().unwrap_or_else(PoisonError::into_inner);
    (seen.failed, seen.errors) = (true, errors);
}

// ── Public API ────────────────────────────
/// Saves the playlist's videos and records each one. Fails only when nothing was saved or skipped.
pub(super) async fn run(toolbox: &Toolbox, order: PlaylistOrder, job: &Job) -> Result<Saved, String> {
    let info = Info::parse(&order.info_json)?;
    let mut fields = info.fields();
    fields.playlist = Some(info.title());
    let template = if order.preset.audio { AUDIO_TEMPLATE } else { VIDEO_TEMPLATE };
    let folder = order.root.join(template::render(template, &fields));
    fs::create_dir_all(&folder).map_err(|e| format!("Can't create the folder: {e}"))?;

    // Kept until this function returns, so the file is deleted when yt-dlp is done, stopped or failed.
    let cookie_file = order.cookies.as_deref().map(CookieFile::write).transpose()?;
    let seen = Mutex::new(Seen::default());
    match &order.links {
        Some(links) => {
            let links = picked(links, order.limit)?;
            let total = Some(links.len() as u64);
            seen.lock().unwrap_or_else(PoisonError::into_inner).total = total;
            job.items(0, total, None);
            // Collected first: a lazy map here makes the job's future not `Send` (a known compiler limit).
            let runs: Vec<_> = links.iter().map(|link| {
                let command = command(toolbox, &order, &folder, cookie_file.as_ref()).map(|mut command| {
                    command.args(["--", link]);
                    command
                });
                run_one(command, &order.slots, &seen, job)
            }).collect();
            // No more waiting than there are slots, so a long list doesn't line up ahead of downloads queued after it.
            stream::iter(runs).buffer_unordered(order.slots.size().max(1)).collect::<Vec<()>>().await;
        }
        None => {
            let mut command = command(toolbox, &order, &folder, cookie_file.as_ref())?;
            command.arg("--yes-playlist");
            if let Some(limit) = order.limit {
                command.args(["--playlist-items", &format!("1-{limit}")]);
            }
            command.args(["--", &order.url]);
            seen.lock().unwrap_or_else(PoisonError::into_inner).total = order.limit.map(u64::from);
            run_one(Ok(command), &order.slots, &seen, job).await;
        }
    }
    if job.cancelled() {
        return Err("Cancelled.".into());
    }
    let Seen { saved, skipped, failed, errors, .. } = seen.into_inner().unwrap_or_else(PoisonError::into_inner);
    if saved.is_empty() && skipped == 0 {
        return Err(if failed { engine::error_text(&errors) } else { "There was nothing to download.".into() });
    }

    let channel = fields.channel.clone();
    let kind = if order.preset.audio { "audio" } else { "video" };
    let mut bytes = 0;
    let mut finished = Vec::new();
    for file in saved.into_iter().filter(|file| file.is_file()) {
        let (file, media) = finish(toolbox, order.preset, file, job).await?;
        bytes += size_of(&file).unwrap_or(0);
        let found = Found { kind, url: &order.url, site: info.site(), user: channel.clone(), transferred: None };
        record_file(&order.root, &file, found, media)?;
        finished.push(file);
    }
    tracing::info!("Saved {} files from a playlist ({skipped} already saved)", finished.len());
    Ok(collection(&order.root, Some(relative(&order.root, &folder)), &finished, skipped, bytes))
}

#[cfg(test)]
mod tests {
    use super::{item_total, picked};

    #[test]
    fn takes_the_first_web_links() {
        let links = ["https://youtu.be/a", "file:///etc/passwd", "https://youtu.be/b", "https://youtu.be/c"].map(String::from);
        assert_eq!(picked(&links, Some(2)).unwrap(), ["https://youtu.be/a", "https://youtu.be/b"]);
        assert_eq!(picked(&links, None).unwrap().len(), 3);
        assert!(picked(&[], None).is_err());
    }

    #[test]
    fn reads_the_playlist_size() {
        assert_eq!(item_total("[download] Downloading item 3 of 40"), Some(40));
        assert_eq!(item_total("[download] Destination: a.mp4"), None);
    }
}

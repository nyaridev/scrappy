//! Playlist: a playlist or channel as one job. yt-dlp saves its videos into `Videos/{channel}/{playlist}/` (or
//! `Audio/…`), skipping videos saved before (its archive in `.scrappy/`) and going on past ones that fail; each saved
//! file then gets the preset's check and encode, and a catalog row. When the window left some videos out, yt-dlp
//! gets the picked ones' links in a temporary file instead of the list's link (ADR-0016).

// ── Imports ───────────────────────────────
use std::fs;
use std::io::Write;
use std::path::PathBuf;

use tempfile::NamedTempFile;

use super::download::{FILE_MARK, Found, Saved, escape_template, finish, record_file, relative};
use super::engine;
use super::presets::Preset;
use super::probe::Info;
use crate::jobs::Job;
use crate::library::template;
use crate::tools::Toolbox;
use crate::vault::{Cookie, CookieFile};

// ── Constants ─────────────────────────────
const VIDEO_TEMPLATE: &str = "Videos/{channel}/{playlist}";
const AUDIO_TEMPLATE: &str = "Audio/{channel}/{playlist}";
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
    /// The links of the videos to save, when some were left out; `None` for the whole list.
    pub only: Option<Vec<String>>,
    /// yt-dlp's JSON from the probe: the playlist's title and channel.
    pub info_json: String,
    pub cookies: Option<Vec<Cookie>>,
}

// ── Private helpers ───────────────────────
/// A file listing the first `limit` of `links` (web links only), one per line, and how many it lists.
fn batch_file(links: &[String], limit: Option<u32>) -> Result<(NamedTempFile, u64), String> {
    let take = limit.map_or(usize::MAX, |limit| limit as usize);
    let links: Vec<&str> = links.iter().filter_map(|link| engine::check_url(link).ok().map(|_| link.as_str())).take(take).collect();
    if links.is_empty() {
        return Err("No videos are picked.".into());
    }
    let mut file = NamedTempFile::new().map_err(|e| format!("Can't write the list of videos: {e}"))?;
    file.write_all(links.join("\n").as_bytes()).map_err(|e| format!("Can't write the list of videos: {e}"))?;
    Ok((file, links.len() as u64))
}

/// The total from `[download] Downloading item 3 of 40`.
fn item_total(line: &str) -> Option<u64> {
    line.strip_prefix(ITEM_TEXT)?.split_once(" of ")?.1.trim().parse().ok()
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
    let mut command = engine::yt_dlp(toolbox)?;
    engine::add_cookies(&mut command, cookie_file.as_ref());
    command
        // `--no-quiet` keeps the "already recorded" lines that `--print` would hide; `-i` goes on past a failed video.
        .args(["--yes-playlist", "--newline", "--no-mtime", "--no-simulate", "--no-quiet", "--no-progress", "-i"])
        .args(["--print", &format!("after_move:{FILE_MARK}%(filepath)s")])
        .args(["-f", &order.preset.format(order.max_height)])
        .args(order.preset.yt_dlp)
        .arg("--download-archive")
        .arg(order.root.join(ARCHIVE))
        .arg("-o")
        .arg(format!("{}/%(title)s [%(id)s].%(ext)s", escape_template(&folder.to_string_lossy())));
    // Kept until this function returns, like the cookies file.
    let batch = order.only.as_deref().map(|links| batch_file(links, order.limit)).transpose()?;
    let mut total = order.limit.map(u64::from);
    match &batch {
        Some((file, count)) => {
            command.arg("-a").arg(file.path());
            total = Some(*count);
        }
        None => {
            if let Some(limit) = order.limit {
                command.args(["--playlist-items", &format!("1-{limit}")]);
            }
            command.args(["--", &order.url]);
        }
    }

    let (mut saved, mut skipped) = (Vec::new(), 0u32);
    let (ok, errors) = engine::run_lines(command, "yt-dlp", job, |line| {
        let name = if let Some(path) = line.strip_prefix(FILE_MARK) {
            let path = PathBuf::from(path.trim());
            let name = path.file_name().map(|name| name.to_string_lossy().into_owned());
            saved.push(path);
            name
        } else if line.contains(SKIPPED_TEXT) {
            skipped += 1;
            None
        } else {
            total = item_total(line).or(total);
            return;
        };
        let done = saved.len() as u64 + u64::from(skipped);
        job.items(done, total, name);
    })
    .await?;
    if saved.is_empty() && skipped == 0 {
        return Err(if ok { "There was nothing to download.".into() } else { engine::error_text(&errors) });
    }

    let channel = fields.channel.clone();
    let kind = if order.preset.audio { "audio" } else { "video" };
    let files = u32::try_from(saved.len()).unwrap_or(u32::MAX);
    for file in saved.into_iter().filter(|file| file.is_file()) {
        let (file, media) = finish(toolbox, order.preset, file, job).await?;
        let found = Found { kind, url: &order.url, site: info.site(), user: channel.clone(), transferred: None };
        record_file(&order.root, &file, found, media)?;
    }
    tracing::info!("Saved {files} files from a playlist ({skipped} already saved)");
    Ok(Saved { path: Some(relative(&order.root, &folder)), files: Some(files), skipped: Some(skipped) })
}

#[cfg(test)]
mod tests {
    use super::{batch_file, item_total};

    #[test]
    fn lists_the_picked_videos() {
        let links = ["https://youtu.be/a", "file:///etc/passwd", "https://youtu.be/b", "https://youtu.be/c"].map(String::from);
        let (file, count) = batch_file(&links, Some(2)).unwrap();
        assert_eq!(count, 2);
        assert_eq!(std::fs::read_to_string(file.path()).unwrap(), "https://youtu.be/a\nhttps://youtu.be/b");
        assert!(batch_file(&[], None).is_err());
    }

    #[test]
    fn reads_the_playlist_size() {
        assert_eq!(item_total("[download] Downloading item 3 of 40"), Some(40));
        assert_eq!(item_total("[download] Destination: a.mp4"), None);
    }
}

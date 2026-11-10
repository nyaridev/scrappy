//! Download: one link with one preset, as a job. yt-dlp fetches and merges from the probe's JSON (no second
//! lookup) with progress, ffprobe checks the result, FFmpeg re-encodes it when the preset needs that, and the catalog
//! records it (ADR-0003, ADR-0004).

// ── Imports ───────────────────────────────
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

use super::engine;
use super::inspect::{self, MediaInfo};
use super::presets::Preset;
use super::probe::Info;
use crate::jobs::{Job, JobPhase};
use crate::library::{self, template};
use crate::tools::{Tool, Toolbox};
use crate::vault::{Cookie, CookieFile};

// ── Constants ─────────────────────────────
/// One machine-readable line per progress report: bytes so far, the size, the estimated size (`NA` when unknown).
const PROGRESS: &str = "download:SCRAPPY %(progress.downloaded_bytes)s %(progress.total_bytes)s \
                        %(progress.total_bytes_estimate)s";
const PROGRESS_MARK: &str = "SCRAPPY ";
/// yt-dlp prints the saved file's path after its last move.
pub(super) const FILE_MARK: &str = "SCRAPPYFILE ";
const VIDEO_TEMPLATE: &str = "Videos/{channel}/{title} [{id}]";
const AUDIO_TEMPLATE: &str = "Audio/{channel}/{title} [{id}]";
const TEMP_DIR: &str = ".scrappy/tmp";

// ── Types ─────────────────────────────────
/// Everything one download needs.
pub(super) struct Order {
    pub root: PathBuf,
    pub url: String,
    pub preset: &'static Preset,
    pub max_height: Option<u32>,
    /// yt-dlp's JSON from the probe.
    pub info_json: String,
    /// Request headers from the extension, already checked; empty for the window's downloads.
    pub headers: Vec<(String, String)>,
    /// The site's sign-in, given once or remembered; written to a temporary file only while yt-dlp runs.
    pub cookies: Option<Vec<Cookie>>,
    /// The file names running downloads have taken, so two never write the same one.
    pub taken: Taken,
    /// Where it goes, relative to the library root and without an extension; `None` for the preset's template.
    pub place: Option<PathBuf>,
    /// The catalog's site; `None` for yt-dlp's name for it.
    pub site: Option<String>,
}

/// Where a saved file came from, for its catalog row.
pub(super) struct Found<'a> {
    /// `video`, `audio` or `social`.
    pub kind: &'static str,
    pub url: &'a str,
    pub site: Option<String>,
    pub user: Option<String>,
    /// Bytes downloaded for it; `None` for its size.
    pub transferred: Option<u64>,
}

/// What a download saved: one file, or a collection's folder and counts.
pub(super) struct Saved {
    /// The file, or the folder a collection went to, relative to the library root.
    pub path: Option<String>,
    /// For a collection: the files saved now.
    pub files: Option<u32>,
    /// For a collection: the files skipped because they were saved before.
    pub skipped: Option<u32>,
}

/// File names (without extension) that running downloads have taken.
pub(super) type Taken = Arc<Mutex<HashSet<PathBuf>>>;

/// Gives a taken name back when the download ends, however it ends.
pub(super) struct Reservation<'a> {
    taken: &'a Taken,
    pub base: PathBuf,
}

/// A running total across streams: yt-dlp downloads video and audio one after the other, each from zero.
#[derive(Default)]
struct Tally {
    base: u64,
    done: u64,
    total: Option<u64>,
}

// ── Private helpers ───────────────────────
/// Bytes so far and the size (exact, else estimated) from one progress line.
fn parse_progress(line: &str) -> Option<(u64, Option<u64>)> {
    let fields: Vec<&str> = line.strip_prefix(PROGRESS_MARK)?.split_whitespace().collect();
    let number = |field: Option<&&str>| field.and_then(|f| f.parse::<f64>().ok()).map(|n| n as u64);
    Some((number(fields.first())?, number(fields.get(1)).or_else(|| number(fields.get(2)))))
}

impl Tally {
    /// Adds a report and returns the running bytes and size.
    fn add(&mut self, done: u64, total: Option<u64>) -> (u64, Option<u64>) {
        if done < self.done {
            self.base += self.total.unwrap_or(self.done);
        }
        (self.done, self.total) = (done, total);
        (self.base + done, total.map(|total| self.base + total))
    }

    fn transferred(&self) -> u64 {
        self.base + self.done
    }
}

/// Whether any file in `base`'s folder is `base` with an extension.
fn on_disk(base: &Path) -> bool {
    let (Some(dir), Some(stem)) = (base.parent(), base.file_name().and_then(|n| n.to_str())) else {
        return false;
    };
    let prefix = format!("{stem}.");
    fs::read_dir(dir).is_ok_and(|entries| {
        entries.filter_map(Result::ok).any(|entry| entry.file_name().to_string_lossy().starts_with(&prefix))
    })
}

/// Takes `base`, or `base (2)`, `base (3)`… when a file or a running download has it. Nothing is overwritten.
pub(super) fn reserve<'a>(taken: &'a Taken, base: &Path) -> Reservation<'a> {
    let mut names = taken.lock().unwrap_or_else(PoisonError::into_inner);
    let free = (1..)
        .map(|n| match n {
            1 => base.to_path_buf(),
            n => PathBuf::from(format!("{} ({n})", base.display())),
        })
        .find(|candidate| !names.contains(candidate) && !on_disk(candidate))
        .unwrap_or_else(|| base.to_path_buf());
    names.insert(free.clone());
    Reservation { taken, base: free }
}

impl Drop for Reservation<'_> {
    fn drop(&mut self) {
        self.taken.lock().unwrap_or_else(PoisonError::into_inner).remove(&self.base);
    }
}

/// yt-dlp reads `%` in an output template as a field; a literal one is written `%%`.
pub(super) fn escape_template(path: &str) -> String {
    path.replace('%', "%%")
}

/// The files yt-dlp leaves when it's stopped: `.part`, `.ytdl` and fragments of this download.
fn leftovers(base: &Path) -> Vec<PathBuf> {
    let (Some(dir), Some(stem)) = (base.parent(), base.file_name().and_then(|n| n.to_str())) else {
        return Vec::new();
    };
    let Ok(entries) = fs::read_dir(dir) else { return Vec::new() };
    entries
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or_default();
            name.starts_with(stem)
                && (name.ends_with(".part") || name.ends_with(".ytdl") || name.contains(".part-Frag"))
        })
        .collect()
}

/// The path relative to `root`, with `/` separators, as the catalog keeps it.
pub(super) fn relative(root: &Path, file: &Path) -> String {
    file.strip_prefix(root).unwrap_or(file).to_string_lossy().replace('\\', "/")
}

/// Runs yt-dlp and returns the saved file and the bytes transferred.
async fn fetch(toolbox: &Toolbox, order: &Order, info_file: &Path, base: &Path, job: &Job) -> Result<(PathBuf, u64), String> {
    // Kept until this function returns, so the file is deleted when yt-dlp is done, stopped or failed.
    let cookie_file = order.cookies.as_deref().map(CookieFile::write).transpose()?;
    let mut command = engine::yt_dlp(toolbox)?;
    engine::add_headers(&mut command, &order.headers);
    engine::add_cookies(&mut command, cookie_file.as_ref());
    command
        .arg("--load-info-json")
        .arg(info_file)
        .args(["--no-simulate", "--newline", "--no-mtime", "--progress", "--progress-template", PROGRESS])
        .args(["--print", &format!("after_move:{FILE_MARK}%(filepath)s")])
        .args(["-f", &order.preset.format(order.max_height)])
        .args(order.preset.yt_dlp)
        .arg("-o")
        .arg(format!("{}.%(ext)s", escape_template(&base.to_string_lossy())));
    let mut tally = Tally::default();
    let mut saved = None;
    let (ok, errors) = engine::run_lines(command, "yt-dlp", job, |line| {
        if let Some((done, total)) = parse_progress(line) {
            let (done, total) = tally.add(done, total);
            job.progress(JobPhase::Download, done, total);
        } else if let Some(path) = line.strip_prefix(FILE_MARK) {
            saved = Some(PathBuf::from(path.trim()));
        }
    })
    .await?;
    if !ok {
        return Err(engine::error_text(&errors));
    }
    let saved = saved.filter(|path| path.is_file()).ok_or("yt-dlp didn't say where it saved the file.")?;
    Ok((saved, tally.transferred()))
}

/// Checks the file with ffprobe and re-encodes it when the preset needs that. Returns the final file and what it
/// holds; without ffprobe the file is kept as it is.
pub(super) async fn finish(toolbox: &Toolbox, preset: &Preset, file: PathBuf, job: &Job) -> Result<(PathBuf, Option<MediaInfo>), String> {
    let Some(ffprobe) = toolbox.program(Tool::Ffmpeg, "ffprobe") else {
        return Ok((file, None));
    };
    let info = inspect::inspect(&ffprobe, &file).await?;
    let ext = file.extension().and_then(|e| e.to_str()).unwrap_or_default().to_string();
    let Some((encode, args)) = preset
        .encode
        .as_ref()
        .and_then(|encode| encode.args(info.video.as_deref(), info.audio.as_deref(), &ext).map(|args| (encode, args)))
    else {
        return Ok((file, Some(info)));
    };
    let ffmpeg = toolbox.locate(Tool::Ffmpeg).1.ok_or("FFmpeg isn't installed. Download it in Settings.")?;
    let output = file.with_extension(format!("encoding.{}", encode.ext));
    inspect::encode(&ffmpeg, &file, &output, &args, info.duration, job).await?;
    let target = file.with_extension(encode.ext);
    fs::remove_file(&file).map_err(|e| format!("Can't replace the download: {e}"))?;
    fs::rename(&output, &target).map_err(|e| format!("Can't replace the download: {e}"))?;
    let info = inspect::inspect(&ffprobe, &target).await?;
    Ok((target, Some(info)))
}

// ── Public API ────────────────────────────
/// Downloads `order` into the library and records it.
pub(super) async fn run(toolbox: &Toolbox, order: Order, job: &Job) -> Result<Saved, String> {
    let info = Info::parse(&order.info_json)?;
    let template = if order.preset.audio { AUDIO_TEMPLATE } else { VIDEO_TEMPLATE };
    let place = order.place.clone().unwrap_or_else(|| template::render(template, &info.fields()));
    let reservation = reserve(&order.taken, &order.root.join(place));
    let base = reservation.base.clone();
    if let Some(dir) = base.parent() {
        fs::create_dir_all(dir).map_err(|e| format!("Can't create the folder: {e}"))?;
    }
    let temp = order.root.join(TEMP_DIR);
    fs::create_dir_all(&temp).map_err(|e| format!("Can't create the library's temp folder: {e}"))?;
    let info_file = temp.join(format!("{}.info.json", job.id()));
    fs::write(&info_file, &order.info_json).map_err(|e| format!("Can't write the download's details: {e}"))?;

    let fetched = fetch(toolbox, &order, &info_file, &base, job).await;
    let _ = fs::remove_file(&info_file);
    let (file, transferred) = match fetched {
        Ok(fetched) => fetched,
        Err(error) => {
            for leftover in leftovers(&base) {
                let _ = fs::remove_file(leftover);
            }
            return Err(error);
        }
    };

    let (file, media) = finish(toolbox, order.preset, file, job).await?;
    let kind = if order.preset.audio { "audio" } else { "video" };
    let site = order.site.clone().or_else(|| info.site());
    let found = Found { kind, url: &order.url, site, user: info.fields().channel, transferred: Some(transferred) };
    let path = record_file(&order.root, &file, found, media)?;
    tracing::info!("Downloaded a {kind} file");
    Ok(Saved { path: Some(path), files: None, skipped: None })
}

/// Adds a saved file to the catalog and returns its path relative to the library root.
pub(super) fn record_file(root: &Path, file: &Path, found: Found<'_>, media: Option<MediaInfo>) -> Result<String, String> {
    let path = relative(root, file);
    let size = fs::metadata(file).map_or(0, |m| m.len());
    let record = library::Record {
        path: path.clone(),
        kind: found.kind,
        source_url: found.url.to_string(),
        site: found.site,
        user: found.user,
        size_bytes: size,
        duration_secs: media.as_ref().and_then(|m| m.duration),
        codec: media.and_then(|m| m.video.or(m.audio)),
        transferred_bytes: found.transferred.unwrap_or(size),
    };
    library::record(root, &record)?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::{Tally, Taken, escape_template, leftovers, parse_progress, relative, reserve};
    use std::fs;
    use std::path::Path;

    #[test]
    fn reads_progress_lines() {
        assert_eq!(parse_progress("SCRAPPY 7168 629172 NA"), Some((7168, Some(629_172))));
        assert_eq!(parse_progress("SCRAPPY 1024 NA 5000.5"), Some((1024, Some(5000))));
        assert_eq!(parse_progress("SCRAPPY NA NA NA"), None);
        assert_eq!(parse_progress("[download] 50%"), None);
    }

    #[test]
    fn keeps_a_running_total_across_streams() {
        let mut tally = Tally::default();
        assert_eq!(tally.add(500, Some(1000)), (500, Some(1000)));
        assert_eq!(tally.add(1000, Some(1000)), (1000, Some(1000)));
        assert_eq!(tally.add(100, Some(300)), (1100, Some(1300)));
        assert_eq!(tally.transferred(), 1100);
    }

    #[test]
    fn escapes_percent_signs() {
        assert_eq!(escape_template("D:/Lib/100% [x]"), "D:/Lib/100%% [x]");
    }

    #[test]
    fn finds_only_this_downloads_leftovers() {
        let dir = tempfile::tempdir().unwrap();
        for name in ["clip [a].mp4.part", "clip [a].f137.mp4.ytdl", "clip [a].mp4", "other.part"] {
            fs::write(dir.path().join(name), "").unwrap();
        }
        let mut found: Vec<String> = leftovers(&dir.path().join("clip [a]"))
            .into_iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        found.sort();
        assert_eq!(found, ["clip [a].f137.mp4.ytdl", "clip [a].mp4.part"]);
    }

    #[test]
    fn never_reuses_a_name() {
        let dir = tempfile::tempdir().unwrap();
        let taken = Taken::default();
        let base = dir.path().join("clip [a]");
        fs::write(dir.path().join("clip [a].mp4"), "").unwrap();
        let first = reserve(&taken, &base);
        let second = reserve(&taken, &base);
        assert_eq!(first.base, dir.path().join("clip [a] (2)"));
        assert_eq!(second.base, dir.path().join("clip [a] (3)"));
        drop(first);
        assert_eq!(reserve(&taken, &base).base, dir.path().join("clip [a] (2)"));
    }

    #[test]
    fn stores_paths_with_forward_slashes() {
        let root = Path::new("D:/Lib");
        assert_eq!(relative(root, &root.join("Videos").join("a.mp4")), "Videos/a.mp4");
    }
}

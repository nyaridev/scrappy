//! Download: one link with one preset, as a job. yt-dlp fetches and merges from the probe's JSON (no second
//! lookup) with progress, ffprobe checks the result, FFmpeg re-encodes it when the preset needs that, and the catalog
//! records it (ADR-0003, ADR-0004). Before that, the same JSON gives the size the preset's pick will download
//! (ADR-0017).

// ── Imports ───────────────────────────────
use std::collections::HashSet;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

use tempfile::NamedTempFile;

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
/// The picked streams' size (exact, else yt-dlp's estimate), their bitrate in kbit/s and the length in seconds; `NA`
/// for what's unknown. HLS streams (X) have only the bitrate.
const SIZE: &str = "%(filesize,filesize_approx)s %(tbr)s %(duration)s";
/// yt-dlp prints the saved file's path after its last move.
pub(super) const FILE_MARK: &str = "SCRAPPYFILE ";
const TEMP_DIR: &str = ".scrappy/tmp";
/// yt-dlp's error when it can't take the audio out of a download, as when the download has none.
const NO_AUDIO_ERROR: &str = "unable to obtain file audio codec";
/// The row's note for a download kept as it is because it has no audio.
const NO_AUDIO_NOTE: &str = "It has no audio, so it was kept as downloaded.";

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
    /// Bytes on disk of what was saved now.
    pub bytes: Option<u64>,
    /// For files kept as they are (no preset): their types, most common first.
    pub types: Vec<String>,
    /// A note on a download that worked, but not as asked (no audio to take out).
    pub warning: Option<String>,
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
/// The size from yt-dlp's `SIZE` line: the size it gave, else bitrate × length, as its own format list does.
fn parse_size(output: &str) -> Option<u64> {
    let fields: Vec<Option<f64>> = output.lines().next()?.split_whitespace().map(|f| f.parse().ok()).collect();
    let field = |index: usize| fields.get(index).copied().flatten();
    let bytes = field(0).or_else(|| Some(field(1)? * 1000.0 / 8.0 * field(2)?))?;
    Some(bytes as u64)
}

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

/// Whether `name` is a file yt-dlp leaves while it works: `.part`, `.ytdl` or a fragment.
fn is_leftover(name: &str) -> bool {
    name.ends_with(".part") || name.ends_with(".ytdl") || name.contains(".part-Frag")
}

/// The files in `base`'s folder named `base` and an extension, with whether each is a leftover.
fn named_like(base: &Path) -> Vec<(PathBuf, bool)> {
    let (Some(dir), Some(stem)) = (base.parent(), base.file_name().and_then(|n| n.to_str())) else {
        return Vec::new();
    };
    let Ok(entries) = fs::read_dir(dir) else { return Vec::new() };
    let prefix = format!("{stem}.");
    entries
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter_map(|path| {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or_default().to_string();
            name.starts_with(&prefix).then(|| (path, is_leftover(&name)))
        })
        .collect()
}

/// The files yt-dlp leaves when it's stopped: `.part`, `.ytdl` and fragments of this download.
fn leftovers(base: &Path) -> Vec<PathBuf> {
    named_like(base).into_iter().filter(|(_, leftover)| *leftover).map(|(path, _)| path).collect()
}

/// After `error`, the file yt-dlp downloaded but couldn't turn into audio, with what it holds, when ffprobe finds
/// no audio in it.
async fn kept_without_audio(toolbox: &Toolbox, base: &Path, error: &str) -> Option<(PathBuf, MediaInfo)> {
    if !error.contains(NO_AUDIO_ERROR) {
        return None;
    }
    let file = named_like(base).into_iter().find(|(_, leftover)| !leftover)?.0;
    let ffprobe = toolbox.program(Tool::Ffmpeg, "ffprobe")?;
    let info = inspect::inspect(&ffprobe, &file).await.ok()?;
    info.audio.is_none().then_some((file, info))
}

/// Moves a video kept without audio from the audio folder to where videos go, named as a video would be there; a
/// Grab save keeps its folder. `made` is whether this download created the audio folder: it goes when left empty.
/// Returns where the file is.
fn into_videos(order: &Order, info: &Info, file: PathBuf, made: bool) -> PathBuf {
    if order.place.is_some() || !order.preset.audio {
        return file;
    }
    let place = template::render(&engine::place_template(false), &info.fields());
    let reservation = reserve(&order.taken, &order.root.join(place));
    let ext = file.extension().map(|ext| ext.to_string_lossy().into_owned()).unwrap_or_default();
    let target = PathBuf::from(format!("{}.{ext}", reservation.base.display()));
    let folder_ready = target.parent().is_some_and(|dir| fs::create_dir_all(dir).is_ok());
    let moved = folder_ready && fs::rename(&file, &target).is_ok();
    if !moved {
        return file;
    }
    if made && let Some(dir) = file.parent() {
        // Only an empty folder goes; anything else in it stays.
        let _ = fs::remove_dir(dir);
    }
    target
}

/// Where `order` saves, relative to the library root and without an extension.
fn place(order: &Order, info: &Info) -> PathBuf {
    let template = engine::place_template(order.preset.audio);
    order.place.clone().unwrap_or_else(|| template::render(&template, &info.fields()))
}

/// The most common types among `files`, as upper-case extensions (`MP4`), most common first, at most three.
pub(super) fn types_of(files: &[PathBuf]) -> Vec<String> {
    let mut counts: Vec<(String, usize)> = Vec::new();
    for extension in files.iter().filter_map(|file| file.extension()) {
        let extension = extension.to_string_lossy().to_uppercase();
        match counts.iter_mut().find(|(seen, _)| *seen == extension) {
            Some((_, count)) => *count += 1,
            None => counts.push((extension, 1)),
        }
    }
    // Stable: a tie keeps the order the files came in.
    counts.sort_by_key(|(_, count)| std::cmp::Reverse(*count));
    counts.into_iter().take(3).map(|(extension, _)| extension).collect()
}

/// A collection's result: its folder and counts, or, when it saved one file and skipped none, that file, as a single
/// download reads.
pub(super) fn collection(root: &Path, folder: Option<String>, saved: &[PathBuf], skipped: u32, bytes: u64) -> Saved {
    let bytes = Some(bytes);
    match saved {
        [file] if skipped == 0 => {
            let path = Some(relative(root, file));
            Saved { path, files: None, skipped: None, bytes, types: Vec::new(), warning: None }
        }
        _ => {
            let files = Some(u32::try_from(saved.len()).unwrap_or(u32::MAX));
            Saved { path: folder, files, skipped: Some(skipped), bytes, types: Vec::new(), warning: None }
        }
    }
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
    engine::add_extras(&mut command, order.preset.audio);
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
/// Where a single video goes with the name `template` (ADR-0034), shown with a made-up video.
pub fn example_name(template: &str) -> String {
    let fields = template::Fields {
        title: Some("How rockets land".into()),
        id: Some("dQw4w9WgXcQ".into()),
        channel: Some("Space Lab".into()),
        playlist: None,
        upload_date: Some("20260412".into()),
        site: Some("youtube.com".into()),
        user: Some("spacelab".into()),
    };
    let path = template::render(&format!("Videos/{template}"), &fields);
    format!("{}.mp4", path.to_string_lossy().replace('\\', "/"))
}

/// The folder `order` saves into, relative to the library root; `None` when its details can't be read.
pub(super) fn folder(order: &Order) -> Option<String> {
    let info = Info::parse(&order.info_json).ok()?;
    let folder = place(order, &info).parent()?.to_string_lossy().replace('\\', "/");
    (!folder.is_empty()).then_some(folder)
}

/// Downloads `order` into the library and records it.
pub(super) async fn run(toolbox: &Toolbox, order: Order, job: &Job) -> Result<Saved, String> {
    let info = Info::parse(&order.info_json)?;
    let reservation = reserve(&order.taken, &order.root.join(place(&order, &info)));
    let base = reservation.base.clone();
    let made = base.parent().is_some_and(|dir| !dir.exists());
    if let Some(dir) = base.parent() {
        fs::create_dir_all(dir).map_err(|e| format!("Can't create the folder: {e}"))?;
    }
    let temp = order.root.join(TEMP_DIR);
    fs::create_dir_all(&temp).map_err(|e| format!("Can't create the library's temp folder: {e}"))?;
    let info_file = temp.join(format!("{}.info.json", job.id()));
    fs::write(&info_file, &order.info_json).map_err(|e| format!("Can't write the download's details: {e}"))?;

    let fetched = fetch(toolbox, &order, &info_file, &base, job).await;
    let _ = fs::remove_file(&info_file);
    let (file, media, transferred, warning) = match fetched {
        Ok((file, transferred)) => {
            let (file, media) = finish(toolbox, order.preset, file, job).await?;
            (file, media, transferred, None)
        }
        // Turning it into audio failed because it has none: it's kept as it came, with the videos.
        Err(error) => match kept_without_audio(toolbox, &base, &error).await {
            Some((file, media)) => {
                let file = into_videos(&order, &info, file, made);
                let transferred = size_of(&file).unwrap_or(0);
                (file, Some(media), transferred, Some(NO_AUDIO_NOTE.to_string()))
            }
            None => {
                for leftover in leftovers(&base) {
                    let _ = fs::remove_file(leftover);
                }
                return Err(error);
            }
        },
    };

    let kind = if order.preset.audio && warning.is_none() { "audio" } else { "video" };
    let site = order.site.clone().or_else(|| info.site());
    let found = Found { kind, url: &order.url, site, user: info.fields().channel, transferred: Some(transferred) };
    let path = record_file(&order.root, &file, found, media)?;
    tracing::info!("Downloaded a {kind} file");
    Ok(Saved { path: Some(path), files: None, skipped: None, bytes: size_of(&file), types: Vec::new(), warning })
}

/// The size `preset` at `max_height` will download for the video in `info_json` (a probe's JSON), from yt-dlp's
/// own format choice run again without the network. `None` when the site doesn't say.
pub(super) async fn estimate(
    toolbox: &Toolbox,
    preset: &Preset,
    max_height: Option<u32>,
    info_json: &str,
) -> Result<Option<u64>, String> {
    let mut info_file = NamedTempFile::new().map_err(|e| format!("Can't write the video's details: {e}"))?;
    info_file.write_all(info_json.as_bytes()).map_err(|e| format!("Can't write the video's details: {e}"))?;
    let mut command = engine::yt_dlp(toolbox)?;
    command
        .arg("--load-info-json")
        .arg(info_file.path())
        .args(["--simulate", "--print", SIZE, "-f", &preset.format(max_height)])
        .args(preset.yt_dlp);
    let output = command.output().await.map_err(|e| format!("yt-dlp didn't start: {e}"))?;
    if !output.status.success() {
        let errors: Vec<String> = String::from_utf8_lossy(&output.stderr).lines().map(str::to_string).collect();
        return Err(engine::error_text(&errors));
    }
    Ok(parse_size(&String::from_utf8_lossy(&output.stdout)))
}

/// A file's size on disk, if it can be read.
pub(super) fn size_of(file: &Path) -> Option<u64> {
    fs::metadata(file).ok().map(|meta| meta.len())
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
    use std::path::PathBuf;
    use super::{
        Info, Order, Tally, Taken, collection, example_name, escape_template, into_videos, leftovers, named_like, parse_progress,
        parse_size, place, relative, reserve, types_of,
    };
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
    fn reads_the_size() {
        assert_eq!(parse_size("288387264 2285.5 635\n"), Some(288_387_264));
        assert_eq!(parse_size("30767602.5 NA NA"), Some(30_767_602));
        assert_eq!(parse_size("NA 601.6 47.48\n"), Some(3_570_495));
        assert_eq!(parse_size("NA NA 47.48\n"), None);
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
        // The finished file is the one that isn't a leftover: kept when it can't become audio.
        let finished: Vec<_> = named_like(&dir.path().join("clip [a]")).into_iter().filter(|(_, left)| !left).collect();
        assert_eq!(finished, [(dir.path().join("clip [a].mp4"), false)]);
    }

    #[test]
    fn moves_a_video_kept_without_audio_to_the_videos() {
        let dir = tempfile::tempdir().unwrap();
        let json = r#"{"id":"x1","title":"Clip","channel":"someone","extractor_key":"Twitter"}"#;
        let preset = super::super::presets::PRESETS.iter().find(|preset| preset.audio).unwrap();
        let root = dir.path().to_path_buf();
        let (url, info_json, taken) = ("https://x.com/a/status/1".to_string(), json.to_string(), Taken::default());
        let (headers, cookies, site) = (Vec::new(), None, None);
        let order = Order { root, url, preset, max_height: None, info_json, headers, cookies, taken, place: None, site };
        let info = Info::parse(json).unwrap();
        let in_audio = dir.path().join(place(&order, &info));
        fs::create_dir_all(in_audio.parent().unwrap()).unwrap();
        let file = PathBuf::from(format!("{}.mp4", in_audio.display()));
        fs::write(&file, "video").unwrap();
        let moved = into_videos(&order, &info, file, true);
        assert!(moved.starts_with(dir.path().join("Videos")) && moved.is_file(), "{moved:?}");
        assert!(!in_audio.parent().unwrap().exists(), "the empty audio folder goes");
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

    #[test]
    fn names_the_common_types_and_reads_one_file_as_a_file() {
        let files: Vec<PathBuf> = ["a.mp4", "b.jpg", "c.MP4", "d.png", "e.webp", "f.jpg", "g.mp4", "no-extension"]
            .iter()
            .map(PathBuf::from)
            .collect();
        assert_eq!(types_of(&files), ["MP4", "JPG", "PNG"]);
        let root = Path::new("/lib");
        let one = collection(root, Some("Social/x/a".into()), &[PathBuf::from("/lib/Social/x/a/v.mp4")], 0, 9);
        assert_eq!((one.path.as_deref(), one.files), (Some("Social/x/a/v.mp4"), None));
        let again = collection(root, Some("Social/x/a".into()), &[PathBuf::from("/lib/Social/x/a/v.mp4")], 2, 9);
        assert_eq!((again.path.as_deref(), again.files, again.skipped), (Some("Social/x/a"), Some(1), Some(2)));
    }

    #[test]
    fn shows_a_name_template_with_a_made_up_video() {
        assert_eq!(example_name("{channel}/{title} [{id}]"), "Videos/Space Lab/How rockets land [dQw4w9WgXcQ].mp4");
        let by_site = "Videos/youtube.com/Space Lab/How rockets land [dQw4w9WgXcQ].mp4";
        assert_eq!(example_name("{site}/{channel}/{title} [{id}]"), by_site);
        assert_eq!(example_name("/{upload_date} {title}/../x"), "Videos/20260412 How rockets land/untitled/x.mp4");
    }
}

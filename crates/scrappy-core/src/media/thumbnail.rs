//! Thumbnail: a small JPEG of a library file for the Gallery, made once with FFmpeg (an image scaled down, a video's
//! frame a few seconds in) and kept in `.scrappy/thumbs/`. The window gets it as a `data:` URL, so its CSP needs no
//! asset protocol (ADR-0005). An SVG is sent as it is. A link whose listing has videos but no picture (a post on X)
//! gets its first video's first frame the same way, straight from the web, and so does each of Grab's finds.

// ── Imports ───────────────────────────────
use std::ffi::OsStr;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use sha2::{Digest, Sha256};
use tokio::sync::Semaphore;

use super::{engine, probe};
use crate::library;
use crate::tools::{self, Tool, Toolbox};

// ── Constants ─────────────────────────────
const DIR: &str = ".scrappy/thumbs";
const WIDTH: u32 = 320;
const TIMEOUT: Duration = Duration::from_secs(30);
/// How many FFmpeg runs make thumbnails at once; a gallery page asks for dozens.
const AT_ONCE: usize = 3;
/// How many web links are read at once for thumbnails; a page of Grab's list asks for up to a hundred.
const WEB_AT_ONCE: usize = 8;
/// Where in a video the frame is taken; a shorter video gives its first frame.
const VIDEO_AT: &str = "3";
const SVG_MAX_BYTES: u64 = 1_000_000;
const VIDEO: &[&str] = &["mp4", "m4v", "mov", "mkv", "webm", "avi", "flv", "wmv", "ogv", "3gp"];
const IMAGE: &[&str] = &["jpg", "jpeg", "png", "webp", "gif", "avif", "bmp", "tif", "tiff", "heic", "jxl", "ico"];

// ── Types ─────────────────────────────────
/// Makes and keeps thumbnails, a few at a time.
#[derive(Clone)]
pub struct Thumbnails {
    toolbox: Toolbox,
    gate: Arc<Semaphore>,
    web: Arc<Semaphore>,
}

// ── Private helpers ───────────────────────
fn data_url(mime: &str, bytes: &[u8]) -> String {
    format!("data:{mime};base64,{}", STANDARD.encode(bytes))
}

/// Whether `thumb` was made after `file` last changed.
fn fresh(thumb: &Path, file: &Path) -> bool {
    let modified = |path: &Path| fs::metadata(path).and_then(|m| m.modified()).ok();
    matches!((modified(thumb), modified(file)), (Some(thumb), Some(file)) if thumb >= file)
}

/// Runs FFmpeg once: one frame of `input` (a file, or a web link), from `at` seconds in, scaled to `WIDTH` at most,
/// into `out`.
async fn frame(ffmpeg: &Path, input: &OsStr, at: Option<&str>, out: &Path) -> bool {
    let mut command = tools::command(ffmpeg);
    // A link can't make FFmpeg open anything but the web (a playlist naming `file:` or `pipe:`), nor a file anything
    // but files.
    let web = input.to_str().is_some_and(|input| input.starts_with("http"));
    let protocols = if web { "http,https,tcp,tls,crypto" } else { "file" };
    command.args(["-hide_banner", "-loglevel", "error", "-nostdin", "-protocol_whitelist", protocols]);
    if let Some(at) = at {
        command.args(["-ss", at]);
    }
    command
        .arg("-i")
        .arg(input)
        .args(["-frames:v", "1", "-vf", &format!("scale='min({WIDTH},iw)':-2"), "-q:v", "5", "-update", "1", "-y"])
        .arg(out);
    let ran = tokio::time::timeout(TIMEOUT, command.output()).await;
    matches!(ran, Ok(Ok(output)) if output.status.success()) && out.is_file()
}

/// One frame of the web link `url` (already checked), from `at` seconds in, as a `data:` URL.
async fn web_frame(ffmpeg: &Path, url: &str, at: Option<&str>) -> Option<String> {
    let dir = tempfile::tempdir().ok()?;
    let out = dir.path().join("frame.jpg");
    if !frame(ffmpeg, OsStr::new(url), at, &out).await {
        return None;
    }
    fs::read(&out).ok().map(|bytes| data_url("image/jpeg", &bytes))
}

// ── Public API ────────────────────────────
/// The first frame of the video at `url` (http or https only), as a `data:` URL; `None` without FFmpeg, or when the
/// video can't be read.
pub(super) async fn video_frame(toolbox: &Toolbox, url: &str) -> Option<String> {
    let url = engine::check_url(url).ok()?;
    web_frame(&toolbox.locate(Tool::Ffmpeg).1?, &url, None).await
}

impl Thumbnails {
    /// Thumbnails made with FFmpeg from `toolbox`.
    pub fn new(toolbox: Toolbox) -> Self {
        Self { toolbox, gate: Arc::new(Semaphore::new(AT_ONCE)), web: Arc::new(Semaphore::new(WEB_AT_ONCE)) }
    }

    /// The thumbnail of a picture or video on the web (http or https only), as a `data:` URL: a video's frame a few
    /// seconds in (else its first), a picture scaled down, or sent as it is when FFmpeg can't read it (an SVG).
    /// `None` when it can't be read.
    pub async fn of_link(&self, url: &str, video: bool) -> Option<String> {
        let url = engine::check_url(url).ok()?;
        let _turn = self.web.acquire().await.ok()?;
        if let Some(ffmpeg) = self.toolbox.locate(Tool::Ffmpeg).1 {
            let at = video.then_some(VIDEO_AT);
            if let Some(found) = web_frame(&ffmpeg, &url, at).await {
                return Some(found);
            }
            if video {
                return web_frame(&ffmpeg, &url, None).await;
            }
        }
        if video { None } else { probe::thumbnail(Some(&url)).await }
    }

    /// The thumbnail of the library file at `path` (relative to `root`), as a `data:` URL. `None` for a file with no
    /// picture (audio, documents) or one FFmpeg can't read.
    pub async fn get(&self, root: &Path, path: &str) -> Result<Option<String>, String> {
        let file = library::locate(root, path)?;
        let ext = file.extension().and_then(|e| e.to_str()).unwrap_or_default().to_ascii_lowercase();
        if ext == "svg" {
            let small = fs::metadata(&file).is_ok_and(|m| m.len() <= SVG_MAX_BYTES);
            return Ok(small.then(|| fs::read(&file).ok()).flatten().map(|bytes| data_url("image/svg+xml", &bytes)));
        }
        let video = VIDEO.contains(&ext.as_str());
        if !video && !IMAGE.contains(&ext.as_str()) {
            return Ok(None);
        }
        let dir = root.join(DIR);
        let thumb = dir.join(format!("{:x}.jpg", Sha256::digest(path.as_bytes())));
        if !fresh(&thumb, &file) {
            let Some(ffmpeg) = self.toolbox.locate(Tool::Ffmpeg).1 else { return Ok(None) };
            let _turn = self.gate.acquire().await.map_err(|_| "Thumbnails stopped.".to_string())?;
            fs::create_dir_all(&dir).map_err(|e| format!("Can't save thumbnails: {e}"))?;
            let part = thumb.with_extension("part.jpg");
            let input = file.as_os_str();
            let made = if video {
                frame(&ffmpeg, input, Some(VIDEO_AT), &part).await || frame(&ffmpeg, input, None, &part).await
            } else {
                frame(&ffmpeg, input, None, &part).await
            };
            if !made || fs::rename(&part, &thumb).is_err() {
                let _ = fs::remove_file(&part);
                return Ok(None);
            }
        }
        Ok(fs::read(&thumb).ok().map(|bytes| data_url("image/jpeg", &bytes)))
    }
}

#[cfg(test)]
mod tests {
    use super::{Thumbnails, fresh};
    use crate::tools::Toolbox;
    use std::fs;

    #[tokio::test]
    async fn skips_files_without_a_picture_and_refuses_paths_outside() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::write(root.join("song.mp3"), "x").unwrap();
        fs::write(root.join("logo.svg"), "<svg xmlns='http://www.w3.org/2000/svg'/>").unwrap();
        let thumbnails = Thumbnails::new(Toolbox::new(root.join("tools")));
        assert_eq!(thumbnails.get(root, "song.mp3").await.unwrap(), None);
        let svg = thumbnails.get(root, "logo.svg").await.unwrap().unwrap();
        assert!(svg.starts_with("data:image/svg+xml;base64,"));
        assert!(thumbnails.get(root, "../outside.jpg").await.is_err());
    }

    /// Makes a 2-second video and a large PNG with FFmpeg, then thumbnails both. `SCRAPPY_TOOLS` names a tools folder
    /// that has FFmpeg; run with `cargo test -p scrappy-core -- --ignored`.
    #[tokio::test]
    #[ignore = "needs FFmpeg in SCRAPPY_TOOLS"]
    async fn makes_thumbnails_with_ffmpeg() {
        let toolbox = Toolbox::new(std::env::var("SCRAPPY_TOOLS").expect("SCRAPPY_TOOLS").into());
        let ffmpeg = toolbox.locate(crate::tools::Tool::Ffmpeg).1.expect("FFmpeg");
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let sources = [("testsrc=duration=2:size=640x360:rate=10", "20", "clip.mp4"), ("testsrc=size=1600x900", "1", "big.png")];
        for (source, frames, name) in sources {
            let made = crate::tools::command(&ffmpeg)
                .args(["-hide_banner", "-loglevel", "error", "-f", "lavfi", "-i", source, "-frames:v", frames, "-y"])
                .arg(root.join(name))
                .status()
                .await
                .unwrap();
            assert!(made.success());
        }
        let thumbnails = Thumbnails::new(toolbox);
        for name in ["clip.mp4", "big.png"] {
            let url = thumbnails.get(root, name).await.unwrap().expect(name);
            assert!(url.starts_with("data:image/jpeg;base64,"));
            // Kept: asking again reads the saved one.
            assert_eq!(thumbnails.get(root, name).await.unwrap().unwrap(), url);
        }
        assert_eq!(fs::read_dir(root.join(".scrappy/thumbs")).unwrap().count(), 2);
    }

    /// Takes the first frame of a short public video straight from the web, and refuses a local path. Needs
    /// `SCRAPPY_TOOLS` with FFmpeg and the network.
    #[tokio::test]
    #[ignore = "needs FFmpeg in SCRAPPY_TOOLS and the network"]
    async fn frames_a_web_video() {
        let toolbox = Toolbox::new(std::env::var("SCRAPPY_TOOLS").expect("SCRAPPY_TOOLS").into());
        let picture = super::video_frame(&toolbox, "https://www.w3schools.com/html/mov_bbb.mp4").await.unwrap();
        assert!(picture.starts_with("data:image/jpeg;base64,"));
        assert_eq!(super::video_frame(&toolbox, "file:///C:/Windows/win.ini").await, None);
        // Grab's finds: a video, a picture scaled down, and an SVG FFmpeg can't read, sent as it is.
        let thumbnails = Thumbnails::new(toolbox);
        let video = thumbnails.of_link("https://www.w3schools.com/html/mov_bbb.mp4", true).await.unwrap();
        assert!(video.starts_with("data:image/jpeg;base64,"));
        let picture = thumbnails.of_link("https://www.w3schools.com/html/pic_trulli.jpg", false).await.unwrap();
        assert!(picture.starts_with("data:image/jpeg;base64,"));
        let svg = "https://upload.wikimedia.org/wikipedia/commons/0/02/SVG_logo.svg";
        assert!(thumbnails.of_link(svg, false).await.unwrap().starts_with("data:image/svg+xml;base64,"));
    }

    #[test]
    fn remakes_a_thumbnail_older_than_its_file() {
        let dir = tempfile::tempdir().unwrap();
        let (file, thumb) = (dir.path().join("a.jpg"), dir.path().join("t.jpg"));
        fs::write(&file, "x").unwrap();
        assert!(!fresh(&thumb, &file));
        fs::write(&thumb, "x").unwrap();
        assert!(fresh(&thumb, &file));
    }
}

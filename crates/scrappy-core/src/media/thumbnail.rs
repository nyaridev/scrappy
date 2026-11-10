//! Thumbnail: a small JPEG of a library file for the Gallery, made once with FFmpeg (an image scaled down, a video's
//! frame a few seconds in) and kept in `.scrappy/thumbs/`. The window gets it as a `data:` URL, so its CSP needs no
//! asset protocol (ADR-0005). An SVG is sent as it is.

// ── Imports ───────────────────────────────
use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use sha2::{Digest, Sha256};
use tokio::sync::Semaphore;

use crate::library;
use crate::tools::{self, Tool, Toolbox};

// ── Constants ─────────────────────────────
const DIR: &str = ".scrappy/thumbs";
const WIDTH: u32 = 320;
const TIMEOUT: Duration = Duration::from_secs(30);
/// How many FFmpeg runs make thumbnails at once; a gallery page asks for dozens.
const AT_ONCE: usize = 3;
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

/// Runs FFmpeg once: one frame of `file`, from `at` seconds in, scaled to `WIDTH` at most, into `out`.
async fn frame(ffmpeg: &Path, file: &Path, at: Option<&str>, out: &Path) -> bool {
    let mut command = tools::command(ffmpeg);
    command.args(["-hide_banner", "-loglevel", "error", "-nostdin"]);
    if let Some(at) = at {
        command.args(["-ss", at]);
    }
    command
        .arg("-i")
        .arg(file)
        .args(["-frames:v", "1", "-vf", &format!("scale='min({WIDTH},iw)':-2"), "-q:v", "5", "-update", "1", "-y"])
        .arg(out);
    let ran = tokio::time::timeout(TIMEOUT, command.output()).await;
    matches!(ran, Ok(Ok(output)) if output.status.success()) && out.is_file()
}

// ── Public API ────────────────────────────
impl Thumbnails {
    /// Thumbnails made with FFmpeg from `toolbox`.
    pub fn new(toolbox: Toolbox) -> Self {
        Self { toolbox, gate: Arc::new(Semaphore::new(AT_ONCE)) }
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
            let made = if video {
                frame(&ffmpeg, &file, Some(VIDEO_AT), &part).await || frame(&ffmpeg, &file, None, &part).await
            } else {
                frame(&ffmpeg, &file, None, &part).await
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

//! Serve: a library video or audio file's bytes for the Gallery's player, a range at a time, through the app's
//! `scrappy:` protocol (ADR-0023). Only video and audio types are served, and only from inside the library.

// ── Imports ───────────────────────────────
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use serde::Serialize;
use ts_rs::TS;

use super::history::locate;

// ── Constants ─────────────────────────────
/// The most bytes one answer carries; the player asks for the next range as it plays or seeks.
const CHUNK: u64 = 4 * 1024 * 1024;
/// What the window's browser plays, by extension, and the type it's sent as.
const PLAYABLE: &[(&str, MediaView, &str)] = &[
    ("mp4", MediaView::Video, "video/mp4"),
    ("m4v", MediaView::Video, "video/mp4"),
    ("webm", MediaView::Video, "video/webm"),
    ("mov", MediaView::Video, "video/quicktime"),
    ("mkv", MediaView::Video, "video/x-matroska"),
    ("ogv", MediaView::Video, "video/ogg"),
    ("mp3", MediaView::Audio, "audio/mpeg"),
    ("m4a", MediaView::Audio, "audio/mp4"),
    ("aac", MediaView::Audio, "audio/aac"),
    ("ogg", MediaView::Audio, "audio/ogg"),
    ("opus", MediaView::Audio, "audio/ogg"),
    ("oga", MediaView::Audio, "audio/ogg"),
    ("flac", MediaView::Audio, "audio/flac"),
    ("wav", MediaView::Audio, "audio/wav"),
    ("weba", MediaView::Audio, "audio/webm"),
];

// ── Types ─────────────────────────────────
/// What the Gallery's viewer does with a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum MediaView {
    /// Shown whole, sent as a `data:` URL.
    Picture,
    /// Played, streamed through `scrappy:`.
    Video,
    Audio,
}

/// One answer to the player: the status (206, or 416 for a range past the end), the type, the range sent and the
/// file's size, and the bytes.
#[derive(Debug, PartialEq, Eq)]
pub struct Served {
    pub status: u16,
    pub mime: &'static str,
    /// `Content-Range`: `bytes 0-99/1000`, or `bytes */1000` for a range past the end.
    pub range: String,
    pub body: Vec<u8>,
}

// ── Private helpers ───────────────────────
/// A link's path back to the file's: `%2F` and other escapes decoded.
fn decoded(text: &str) -> Result<String, String> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        if bytes[at] == b'%' {
            let hex = bytes.get(at + 1..at + 3).and_then(|hex| std::str::from_utf8(hex).ok());
            let byte = hex.and_then(|hex| u8::from_str_radix(hex, 16).ok()).ok_or("Not a library link.")?;
            out.push(byte);
            at += 3;
        } else {
            out.push(bytes[at]);
            at += 1;
        }
    }
    String::from_utf8(out).map_err(|_| "Not a library link.".to_string())
}

/// The first and last byte a `Range` header asks for, in a file of `total` bytes; `None` when it's past the end. No
/// header, or one that can't be read, asks from the start.
fn wanted(header: Option<&str>, total: u64) -> Option<(u64, u64)> {
    let last = total.checked_sub(1)?;
    let spec = header.and_then(|header| header.trim().strip_prefix("bytes=")).and_then(|spec| spec.split(',').next());
    let Some((from, to)) = spec.and_then(|spec| spec.trim().split_once('-')) else { return Some((0, last)) };
    let (start, end) = match (from.trim().parse::<u64>().ok(), to.trim().parse::<u64>().ok()) {
        (Some(start), Some(end)) => (start, end.min(last)),
        (Some(start), None) => (start, last),
        // `bytes=-500`: the last 500 bytes.
        (None, Some(tail)) => (total.saturating_sub(tail), last),
        (None, None) => (0, last),
    };
    (start <= end && start <= last).then_some((start, end))
}

// ── Public API ────────────────────────────
/// What the viewer can do with the library file at `path` of `bytes` bytes, by its extension: show a picture of a
/// web type up to `picture_max` bytes, or play a video or audio type.
pub(super) fn view_of(path: &str, bytes: u64, pictures: &[(&str, &str)], picture_max: u64) -> Option<MediaView> {
    let ext = path.rsplit_once('.').map(|(_, ext)| ext.to_ascii_lowercase())?;
    if pictures.iter().any(|(known, _)| *known == ext) {
        return (bytes <= picture_max).then_some(MediaView::Picture);
    }
    PLAYABLE.iter().find(|(known, ..)| *known == ext).map(|(_, view, _)| *view)
}

/// The bytes the player asks for: the library file at `link` (the protocol link's path, escaped) and the `Range`
/// header. Refuses a file outside the library, or one that isn't video or audio.
pub fn serve(root: &Path, link: &str, range: Option<&str>) -> Result<Served, String> {
    let path = decoded(link.trim_start_matches('/'))?;
    let ext = path.rsplit_once('.').map(|(_, ext)| ext.to_ascii_lowercase()).unwrap_or_default();
    let (_, _, mime) = PLAYABLE.iter().find(|(known, ..)| *known == ext).ok_or("Only video and audio play here.")?;
    let file = locate(root, &path)?;
    let mut opened = File::open(&file).map_err(|e| format!("Can't read the file: {e}"))?;
    let total = opened.metadata().map_err(|e| format!("Can't read the file: {e}"))?.len();
    let Some((start, end)) = wanted(range, total) else {
        return Ok(Served { status: 416, mime, range: format!("bytes */{total}"), body: Vec::new() });
    };
    let end = end.min(start + CHUNK - 1);
    let mut body = vec![0; usize::try_from(end - start + 1).map_err(|_| "Too big a range.".to_string())?];
    opened.seek(SeekFrom::Start(start)).and_then(|_| opened.read_exact(&mut body)).map_err(|e| format!("Can't read the file: {e}"))?;
    Ok(Served { status: 206, mime, range: format!("bytes {start}-{end}/{total}"), body })
}

#[cfg(test)]
mod tests {
    use super::{CHUNK, MediaView, serve, view_of, wanted};
    use std::fs;

    #[test]
    fn reads_a_range_header() {
        assert_eq!(wanted(Some("bytes=0-"), 1000), Some((0, 999)));
        assert_eq!(wanted(Some("bytes=100-199"), 1000), Some((100, 199)));
        assert_eq!(wanted(Some("bytes=900-5000"), 1000), Some((900, 999)));
        assert_eq!(wanted(Some("bytes=-100"), 1000), Some((900, 999)));
        assert_eq!(wanted(None, 1000), Some((0, 999)));
        assert_eq!(wanted(Some("bytes=1000-"), 1000), None);
        assert_eq!(wanted(Some("bytes=0-"), 0), None);
    }

    #[test]
    fn serves_video_in_ranges_from_the_library_only() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join("Videos/A b")).unwrap();
        let bytes: Vec<u8> = (0..=255).cycle().take(CHUNK as usize + 10).collect();
        fs::write(root.join("Videos/A b/clip.mp4"), &bytes).unwrap();
        fs::write(root.join("notes.txt"), "secret").unwrap();

        let first = serve(root, "/Videos%2FA%20b%2Fclip.mp4", Some("bytes=0-")).unwrap();
        assert_eq!((first.status, first.mime, first.body.len() as u64), (206, "video/mp4", CHUNK));
        assert_eq!(first.range, format!("bytes 0-{}/{}", CHUNK - 1, CHUNK + 10));
        let tail = serve(root, "/Videos/A%20b/clip.mp4", Some("bytes=-4")).unwrap();
        assert_eq!(tail.body, bytes[bytes.len() - 4..]);
        assert_eq!(serve(root, "/Videos%2FA%20b%2Fclip.mp4", Some("bytes=99999999-")).unwrap().status, 416);

        assert!(serve(root, "/notes.txt", None).unwrap_err().contains("video and audio"));
        assert!(serve(root, "/..%2F..%2Fsecret.mp4", None).is_err());
        assert!(serve(root, "/bad%zz.mp4", None).is_err());
    }

    #[test]
    fn says_what_the_viewer_does() {
        let pictures = [("jpg", "image/jpeg")];
        assert_eq!(view_of("a/b.JPG", 10, &pictures, 100), Some(MediaView::Picture));
        assert_eq!(view_of("a/b.jpg", 1000, &pictures, 100), None);
        assert_eq!(view_of("a/b.mkv", 1000, &pictures, 100), Some(MediaView::Video));
        assert_eq!(view_of("a/b.opus", 1000, &pictures, 100), Some(MediaView::Audio));
        assert_eq!(view_of("a/b.pdf", 1, &pictures, 100), None);
    }
}

//! Find: one downloadable thing on a page, and sorting a page's requests and links into finds: video, audio, stream
//! (HLS/DASH), image or other file. Pieces of a stream, page code and fonts aren't finds.

// ── Imports ───────────────────────────────
use std::collections::HashMap;

use reqwest::Url;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

// ── Constants ─────────────────────────────
const VIDEO: &[&str] = &["mp4", "m4v", "webm", "mkv", "mov", "avi", "flv", "wmv", "ogv", "3gp"];
const AUDIO: &[&str] = &["mp3", "m4a", "aac", "ogg", "oga", "opus", "wav", "flac", "weba"];
const STREAM: &[&str] = &["m3u8", "mpd"];
const IMAGE: &[&str] = &["jpg", "jpeg", "png", "webp", "gif", "avif", "bmp", "svg", "tif", "tiff", "heic", "jxl"];
const OTHER: &[&str] = &[
    "pdf", "zip", "rar", "7z", "tar", "gz", "xz", "exe", "msi", "dmg", "pkg", "deb", "rpm", "apk", "iso", "torrent",
    "doc", "docx", "xls", "xlsx", "ppt", "pptx", "odt", "epub", "csv", "txt", "psd", "blend", "glb", "fbx", "obj", "stl",
];
/// Pieces of a stream: the playlist is the find, not its segments.
const SEGMENTS: &[&str] = &["ts", "m4s", "m4f", "cmfv", "cmfa", "aac-seg"];
const STREAM_MIMES: &[&str] =
    &["application/vnd.apple.mpegurl", "application/x-mpegurl", "audio/mpegurl", "audio/x-mpegurl", "application/dash+xml"];
/// The most finds one page gives; a page with more is cut there.
pub const MAX_FINDS: usize = 2000;

// ── Types ─────────────────────────────────
/// What a find is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum FindKind {
    Video,
    Audio,
    /// An HLS or DASH playlist: fetched piece by piece and joined.
    Stream,
    Image,
    Other,
}

/// A request or link seen on a page, before it's sorted.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Seen {
    pub url: String,
    /// The response's `Content-Type`, when the request was seen.
    pub mime: Option<String>,
    pub size: Option<u64>,
}

/// One downloadable thing on a page.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Find {
    pub url: String,
    pub kind: FindKind,
    /// The file name in the link, decoded; the host when there's none.
    pub name: String,
    /// Bytes, when the page's response said.
    #[ts(type = "number | null")]
    pub size: Option<u64>,
}

// ── Private helpers ───────────────────────
fn extension_of(url: &Url) -> Option<String> {
    let last = url.path_segments()?.rev().find(|segment| !segment.is_empty())?;
    let (_, ext) = last.rsplit_once('.')?;
    Some(ext.to_ascii_lowercase())
}

fn by_mime(mime: &str) -> Option<FindKind> {
    let mime = mime.split(';').next().unwrap_or(mime).trim().to_ascii_lowercase();
    if STREAM_MIMES.contains(&mime.as_str()) {
        return Some(FindKind::Stream);
    }
    let (family, kind) = mime.split_once('/')?;
    match (family, kind) {
        (_, "mp2t" | "iso.segment") => None,
        ("video", _) => Some(FindKind::Video),
        ("audio", _) => Some(FindKind::Audio),
        ("image", _) => Some(FindKind::Image),
        ("application", "pdf" | "zip" | "x-7z-compressed" | "vnd.rar" | "x-rar-compressed" | "x-msdownload") => {
            Some(FindKind::Other)
        }
        _ => None,
    }
}

fn by_extension(ext: &str) -> Option<FindKind> {
    [(STREAM, FindKind::Stream), (VIDEO, FindKind::Video), (AUDIO, FindKind::Audio), (IMAGE, FindKind::Image), (OTHER, FindKind::Other)]
        .into_iter()
        .find_map(|(list, kind)| list.contains(&ext).then_some(kind))
}

/// The link's file name, decoded: `.../Sunset%20sky.jpg?w=2` → `Sunset sky.jpg`.
fn name_of(url: &Url) -> String {
    let last = url.path_segments().and_then(|mut segments| segments.rfind(|segment| !segment.is_empty()));
    let decoded = last.map(percent_decode).filter(|name| !name.trim().is_empty());
    decoded.unwrap_or_else(|| url.host_str().unwrap_or("file").to_string())
}

fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        let hex = bytes.get(index + 1..index + 3).and_then(|h| std::str::from_utf8(h).ok());
        match (bytes[index], hex.and_then(|h| u8::from_str_radix(h, 16).ok())) {
            (b'%', Some(byte)) => {
                out.push(byte);
                index += 3;
            }
            (byte, _) => {
                out.push(byte);
                index += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

// ── Public API ────────────────────────────
/// What `url` is, from its response's MIME type when that's telling, else its extension. `None` for things that
/// aren't worth downloading on their own: page code, fonts, stream segments, data links.
pub fn classify(url: &str, mime: Option<&str>) -> Option<FindKind> {
    let url = Url::parse(url).ok().filter(|url| matches!(url.scheme(), "http" | "https"))?;
    let ext = extension_of(&url);
    if ext.as_deref().is_some_and(|ext| SEGMENTS.contains(&ext)) {
        return None;
    }
    mime.and_then(by_mime).or_else(|| ext.as_deref().and_then(by_extension))
}

/// A page's requests and links as finds: sorted, one per link (the largest size seen wins), at most `MAX_FINDS`.
pub fn collect(seen: impl IntoIterator<Item = Seen>) -> Vec<Find> {
    let mut finds: HashMap<String, Find> = HashMap::new();
    for item in seen {
        let Some(kind) = classify(&item.url, item.mime.as_deref()) else { continue };
        let Ok(url) = Url::parse(&item.url) else { continue };
        if finds.len() >= MAX_FINDS && !finds.contains_key(&item.url) {
            continue;
        }
        let find = finds.entry(item.url.clone()).or_insert_with(|| Find { name: name_of(&url), url: item.url, kind, size: None });
        find.size = find.size.max(item.size);
        // A response's MIME type beats a guess from a link found in the page.
        if item.mime.is_some() {
            find.kind = kind;
        }
    }
    let mut finds: Vec<Find> = finds.into_values().collect();
    finds.sort_by(|a, b| (a.kind, &a.name, &a.url).cmp(&(b.kind, &b.name, &b.url)));
    finds
}

#[cfg(test)]
mod tests {
    use super::{FindKind, Seen, classify, collect};

    #[test]
    fn sorts_by_type_then_extension() {
        let kind = |url, mime| classify(url, mime);
        assert_eq!(kind("https://cdn.example/v/master.m3u8?t=1", None), Some(FindKind::Stream));
        assert_eq!(kind("https://cdn.example/manifest", Some("application/dash+xml")), Some(FindKind::Stream));
        assert_eq!(kind("https://cdn.example/play?id=4", Some("video/mp4")), Some(FindKind::Video));
        assert_eq!(kind("https://cdn.example/a.MP3", None), Some(FindKind::Audio));
        assert_eq!(kind("https://cdn.example/i/cat.webp", Some("image/webp; q=1")), Some(FindKind::Image));
        assert_eq!(kind("https://example.com/files/manual.pdf", None), Some(FindKind::Other));
    }

    #[test]
    fn skips_what_isnt_a_find() {
        assert_eq!(classify("https://cdn.example/seg-00012.ts", Some("video/mp2t")), None);
        assert_eq!(classify("https://cdn.example/chunk-4.m4s", Some("video/mp4")), None);
        assert_eq!(classify("https://example.com/app.js", Some("text/javascript")), None);
        assert_eq!(classify("https://example.com/font.woff2", None), None);
        assert_eq!(classify("data:image/png;base64,AAAA", None), None);
        assert_eq!(classify("https://example.com/about", Some("text/html")), None);
    }

    #[test]
    fn keeps_one_find_per_link() {
        let seen = |url: &str, mime: Option<&str>, size| Seen { url: url.into(), mime: mime.map(Into::into), size };
        let finds = collect([
            seen("https://example.com/b/Sunset%20sky.jpg", None, None),
            seen("https://example.com/b/Sunset%20sky.jpg", Some("image/jpeg"), Some(2048)),
            seen("https://example.com/v/clip.mp4", Some("video/mp4"), Some(9)),
            seen("https://example.com/app.js", Some("text/javascript"), Some(5)),
            seen("https://example.com/", Some("image/png"), None),
        ]);
        let shown: Vec<_> = finds.iter().map(|f| (f.kind, f.name.as_str(), f.size)).collect();
        assert_eq!(
            shown,
            [
                (FindKind::Video, "clip.mp4", Some(9)),
                (FindKind::Image, "Sunset sky.jpg", Some(2048)),
                (FindKind::Image, "example.com", None),
            ]
        );
    }
}

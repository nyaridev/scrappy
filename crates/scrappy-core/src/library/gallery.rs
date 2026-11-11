//! Gallery: the library folder's files one page at a time for the Gallery view, filtered by kind, site and a search,
//! sorted by date, size or name, and a picture whole for the lightbox. The folder says what's there; the catalog adds
//! what it knows of each file (ADR-0015).

// ── Imports ───────────────────────────────
use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::Path;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::catalog;
use super::disk::{Disk, DiskFile};
use super::history::locate;

// ── Constants ─────────────────────────────
/// The most files one page holds (a reload asks for every row the window shows).
const MAX_PAGE: u32 = 1000;
/// A file's kind by its extension, when the catalog doesn't know it.
const VIDEO: &[&str] = &["mp4", "mkv", "webm", "mov", "avi", "m4v", "flv", "ts", "wmv"];
const AUDIO: &[&str] = &["mp3", "m4a", "opus", "ogg", "flac", "wav", "aac", "wma"];
const IMAGE: &[&str] =
    &["jpg", "jpeg", "png", "webp", "gif", "avif", "bmp", "svg", "tif", "tiff", "heic", "heif", "jxl"];
/// Pictures the window can show, by extension; other images open in their app.
const PICTURES: &[(&str, &str)] = &[
    ("jpg", "image/jpeg"),
    ("jpeg", "image/jpeg"),
    ("png", "image/png"),
    ("webp", "image/webp"),
    ("gif", "image/gif"),
    ("avif", "image/avif"),
    ("bmp", "image/bmp"),
    ("svg", "image/svg+xml"),
];
/// A bigger picture opens in its app instead: it would travel to the window whole.
const PICTURE_MAX_BYTES: u64 = 30_000_000;

// ── Types ─────────────────────────────────
/// How the gallery is sorted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum GallerySort {
    Newest,
    Oldest,
    Largest,
    Name,
}

/// One page of the gallery: what to filter by, how to sort, and where the page starts.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct GalleryQuery {
    pub kind: Option<String>,
    pub site: Option<String>,
    /// Matched anywhere in the file's path, ignoring case.
    pub search: Option<String>,
    pub sort: GallerySort,
    pub offset: u32,
    pub limit: u32,
}

/// One file in the gallery.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct GalleryItem {
    /// Relative to the library root, with `/` separators.
    pub path: String,
    pub name: String,
    pub kind: String,
    pub site: Option<String>,
    pub user: Option<String>,
    #[ts(type = "number")]
    pub bytes: u64,
    /// Seconds.
    pub duration: Option<f64>,
    /// When it was downloaded, or changed on disk when the catalog doesn't know it. Unix seconds.
    #[ts(type = "number")]
    pub downloaded_at: i64,
}

/// A page of files, how many match in all, and the kinds and sites the folder holds (for the filters).
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct GalleryPage {
    pub items: Vec<GalleryItem>,
    pub total: u32,
    pub kinds: Vec<String>,
    pub sites: Vec<String>,
}

/// What the catalog knows of a file.
struct Known {
    kind: String,
    site: Option<String>,
    user: Option<String>,
    duration: Option<f64>,
    downloaded_at: i64,
}

// ── Private helpers ───────────────────────
fn error(e: rusqlite::Error) -> String {
    format!("Can't read the library catalog: {e}")
}

/// What the catalog knows, by path.
fn known(root: &Path) -> Result<HashMap<String, Known>, String> {
    let connection = catalog::open(root)?;
    let mut statement = connection
        .prepare("SELECT path, kind, site, user, duration_secs, downloaded_at FROM downloads")
        .map_err(error)?;
    let rows = statement
        .query_map([], |row| {
            let known = Known {
                kind: row.get(1)?,
                site: row.get(2)?,
                user: row.get(3)?,
                duration: row.get(4)?,
                downloaded_at: row.get(5)?,
            };
            Ok((row.get::<_, String>(0)?, known))
        })
        .map_err(error)?;
    rows.collect::<Result<_, _>>().map_err(error)
}

/// A file's kind from its extension: a mirror's start page or saved page is a `site`.
fn kind_of(path: &str) -> &'static str {
    let ext = path.rsplit_once('.').map(|(_, ext)| ext.to_ascii_lowercase()).unwrap_or_default();
    match ext.as_str() {
        ext if VIDEO.contains(&ext) => "video",
        ext if AUDIO.contains(&ext) => "audio",
        ext if IMAGE.contains(&ext) => "image",
        "html" if path.starts_with("Sites/") => "site",
        _ => "file",
    }
}

fn item(file: &DiskFile, known: Option<&Known>) -> GalleryItem {
    GalleryItem {
        name: file.path.rsplit('/').next().unwrap_or(&file.path).to_string(),
        path: file.path.clone(),
        kind: known.map_or_else(|| kind_of(&file.path).to_string(), |known| known.kind.clone()),
        site: known.and_then(|known| known.site.clone()),
        user: known.and_then(|known| known.user.clone()),
        bytes: file.bytes,
        duration: known.and_then(|known| known.duration),
        downloaded_at: known.map_or(file.modified, |known| known.downloaded_at),
    }
}

// ── Public API ────────────────────────────
/// The page of the library at `root` that `query` asks for, from the files `disk` finds there.
pub fn page(root: &Path, disk: &Disk, query: &GalleryQuery) -> Result<GalleryPage, String> {
    let files = disk.files(root)?;
    let known = known(root)?;
    let all: Vec<GalleryItem> = files.iter().map(|file| item(file, known.get(&file.path))).collect();
    let kinds: BTreeSet<String> = all.iter().map(|item| item.kind.clone()).collect();
    let sites: BTreeSet<String> = all.iter().filter_map(|item| item.site.clone()).collect();

    let kind = query.kind.as_deref().filter(|kind| !kind.is_empty());
    let site = query.site.as_deref().filter(|site| !site.is_empty());
    let search = query.search.as_deref().map(|search| search.trim().to_lowercase()).filter(|search| !search.is_empty());
    let mut items: Vec<GalleryItem> = all
        .into_iter()
        .filter(|item| kind.is_none_or(|kind| item.kind == kind))
        .filter(|item| site.is_none_or(|site| item.site.as_deref() == Some(site)))
        .filter(|item| search.as_deref().is_none_or(|search| item.path.to_lowercase().contains(search)))
        .collect();
    match query.sort {
        GallerySort::Newest => {
            items.sort_by(|a, b| b.downloaded_at.cmp(&a.downloaded_at).then_with(|| a.path.cmp(&b.path)))
        }
        GallerySort::Oldest => {
            items.sort_by(|a, b| a.downloaded_at.cmp(&b.downloaded_at).then_with(|| a.path.cmp(&b.path)))
        }
        GallerySort::Largest => items.sort_by(|a, b| b.bytes.cmp(&a.bytes).then_with(|| a.path.cmp(&b.path))),
        GallerySort::Name => items.sort_by_cached_key(|item| (item.name.to_lowercase(), item.path.clone())),
    }
    let total = u32::try_from(items.len()).unwrap_or(u32::MAX);
    let items = items.into_iter().skip(query.offset as usize).take(query.limit.min(MAX_PAGE) as usize).collect();
    Ok(GalleryPage { items, total, kinds: kinds.into_iter().collect(), sites: sites.into_iter().collect() })
}

/// The library picture at `path` as a `data:` URL, for the lightbox. `None` for a file the window can't show (not a
/// web image type, or over 30 MB).
pub fn picture(root: &Path, path: &str) -> Result<Option<String>, String> {
    let file = locate(root, path)?;
    let ext = file.extension().and_then(|e| e.to_str()).unwrap_or_default().to_ascii_lowercase();
    let Some((_, mime)) = PICTURES.iter().find(|(known, _)| *known == ext) else { return Ok(None) };
    if fs::metadata(&file).map_or(true, |m| m.len() > PICTURE_MAX_BYTES) {
        return Ok(None);
    }
    let bytes = fs::read(&file).map_err(|e| format!("Can't read the picture: {e}"))?;
    Ok(Some(format!("data:{mime};base64,{}", STANDARD.encode(bytes))))
}

#[cfg(test)]
mod tests {
    use super::{GalleryQuery, GallerySort, page, picture};
    use crate::library::catalog::{self, Record};
    use crate::library::disk::Disk;
    use std::fs;
    use std::sync::Arc;

    fn add(root: &std::path::Path, path: &str, kind: &'static str, site: &str, size: u64) {
        let record = Record {
            path: path.into(),
            kind,
            source_url: "https://example.com".into(),
            site: Some(site.into()),
            user: None,
            size_bytes: size,
            duration_secs: None,
            codec: None,
            transferred_bytes: size,
        };
        catalog::record(root, &record).unwrap();
    }

    fn query(sort: GallerySort) -> GalleryQuery {
        GalleryQuery { kind: None, site: None, search: None, sort, offset: 0, limit: 50 }
    }

    #[test]
    fn shows_the_folder_with_what_the_catalog_knows() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join(".scrappy")).unwrap();
        fs::create_dir_all(root.join("Videos/b")).unwrap();
        fs::create_dir_all(root.join("Images/a")).unwrap();
        fs::write(root.join("Images/a/zebra.jpg"), "x".repeat(300)).unwrap();
        fs::write(root.join("Videos/b/Apple 100%.mp4"), "x".repeat(900)).unwrap();
        fs::write(root.join("Videos/b/mango_1.mp4"), "x".repeat(100)).unwrap();
        fs::write(root.join("Videos/b/added by hand.mkv"), "x").unwrap();
        add(root, "Images/a/zebra.jpg", "image", "a", 300);
        add(root, "Videos/b/Apple 100%.mp4", "social", "b", 900);
        add(root, "Videos/b/mango_1.mp4", "video", "b", 100);
        // In the catalog, but deleted: not shown.
        add(root, "Videos/b/gone.mp4", "video", "b", 50);
        let disk = Disk::new(Arc::new(|| {}));
        let names = |query: &GalleryQuery| {
            page(root, &disk, query)
                .unwrap()
                .items
                .into_iter()
                .map(|item| item.name)
                .collect::<Vec<_>>()
        };

        assert_eq!(
            names(&query(GallerySort::Name)),
            [
                "added by hand.mkv",
                "Apple 100%.mp4",
                "mango_1.mp4",
                "zebra.jpg"
            ]
        );
        assert_eq!(names(&query(GallerySort::Largest))[0], "Apple 100%.mp4");
        let videos = GalleryQuery {
            kind: Some("video".into()),
            ..query(GallerySort::Name)
        };
        assert_eq!(
            names(&videos),
            ["added by hand.mkv", "mango_1.mp4"],
            "the catalog's kind wins (social)"
        );
        let percent = GalleryQuery {
            search: Some("100%".into()),
            ..query(GallerySort::Name)
        };
        assert_eq!(names(&percent), ["Apple 100%.mp4"]);
        let site = GalleryQuery {
            site: Some("b".into()),
            ..query(GallerySort::Name)
        };
        assert_eq!(names(&site), ["Apple 100%.mp4", "mango_1.mp4"]);

        let second = GalleryQuery {
            offset: 1,
            limit: 1,
            ..query(GallerySort::Name)
        };
        let found = page(root, &disk, &second).unwrap();
        assert_eq!(
            (found.total, found.items.len(), found.items[0].name.as_str()),
            (4, 1, "Apple 100%.mp4")
        );
        assert_eq!(found.kinds, ["image", "social", "video"]);
        assert_eq!(found.sites, ["a", "b"]);
    }

    #[test]
    fn sends_web_pictures_whole() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::write(root.join("a.PNG"), "png").unwrap();
        fs::write(root.join("b.heic"), "heic").unwrap();
        assert_eq!(picture(root, "a.PNG").unwrap().as_deref(), Some("data:image/png;base64,cG5n"));
        assert_eq!(picture(root, "b.heic").unwrap(), None);
        assert!(picture(root, "../a.png").is_err());
    }
}

//! Gallery: the catalog one page at a time for the Gallery view, filtered by kind, site and a search, sorted by date,
//! size or name, with files that are gone from disk marked, and a picture whole for the lightbox (ADR-0004).

// ── Imports ───────────────────────────────
use std::fs;
use std::path::Path;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use rusqlite::{params_from_iter, types::Value};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::catalog;
use super::history::locate;

// ── Constants ─────────────────────────────
/// The most files one page holds.
const MAX_PAGE: u32 = 200;
/// The file name from a `/`-separated path: `rtrim` with the name's own characters strips it down to the folder, and
/// `replace` takes that folder off. SQLite has no basename function.
const NAME: &str = "replace(path, rtrim(path, replace(path, '/', '')), '')";
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
    /// Unix seconds.
    #[ts(type = "number")]
    pub downloaded_at: i64,
    /// The file is no longer on disk.
    pub missing: bool,
}

/// A page of files and how many match in all.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct GalleryPage {
    pub items: Vec<GalleryItem>,
    pub total: u32,
}

// ── Private helpers ───────────────────────
fn error(e: rusqlite::Error) -> String {
    format!("Can't read the library catalog: {e}")
}

/// `term` as a `LIKE` pattern that matches it literally.
fn like(term: &str) -> String {
    let escaped = term.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_");
    format!("%{escaped}%")
}

// ── Public API ────────────────────────────
/// The page of the library at `root` that `query` asks for.
pub fn page(root: &Path, query: &GalleryQuery) -> Result<GalleryPage, String> {
    let connection = catalog::open(root)?;
    let mut conditions = Vec::new();
    let mut values: Vec<Value> = Vec::new();
    if let Some(kind) = query.kind.as_ref().filter(|kind| !kind.is_empty()) {
        conditions.push("kind = ?");
        values.push(Value::Text(kind.clone()));
    }
    if let Some(site) = query.site.as_ref().filter(|site| !site.is_empty()) {
        conditions.push("site = ?");
        values.push(Value::Text(site.clone()));
    }
    if let Some(search) = query.search.as_deref().map(str::trim).filter(|search| !search.is_empty()) {
        conditions.push("path LIKE ? ESCAPE '\\'");
        values.push(Value::Text(like(search)));
    }
    let filter = if conditions.is_empty() { String::new() } else { format!("WHERE {}", conditions.join(" AND ")) };
    let order = match query.sort {
        GallerySort::Newest => "downloaded_at DESC, id DESC".to_string(),
        GallerySort::Oldest => "downloaded_at, id".to_string(),
        GallerySort::Largest => "size_bytes DESC, id DESC".to_string(),
        GallerySort::Name => format!("{NAME} COLLATE NOCASE, id"),
    };

    let total: i64 = connection
        .query_row(&format!("SELECT COUNT(*) FROM downloads {filter}"), params_from_iter(&values), |row| row.get(0))
        .map_err(error)?;
    // `filter` and `order` are built from this file's text only; every value is a parameter.
    let sql = format!(
        "SELECT path, kind, site, user, size_bytes, duration_secs, downloaded_at FROM downloads {filter}
         ORDER BY {order} LIMIT ? OFFSET ?"
    );
    values.push(Value::Integer(i64::from(query.limit.min(MAX_PAGE))));
    values.push(Value::Integer(i64::from(query.offset)));
    let mut statement = connection.prepare(&sql).map_err(error)?;
    let rows = statement
        .query_map(params_from_iter(&values), |row| {
            let path: String = row.get(0)?;
            Ok(GalleryItem {
                name: path.rsplit('/').next().unwrap_or(&path).to_string(),
                missing: !root.join(&path).is_file(),
                path,
                kind: row.get(1)?,
                site: row.get(2)?,
                user: row.get(3)?,
                bytes: u64::try_from(row.get::<_, i64>(4)?).unwrap_or(0),
                duration: row.get(5)?,
                downloaded_at: row.get(6)?,
            })
        })
        .map_err(error)?;
    let items = rows.collect::<Result<_, _>>().map_err(error)?;
    Ok(GalleryPage { items, total: u32::try_from(total).unwrap_or(u32::MAX) })
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
    use std::fs;

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

    fn names(root: &std::path::Path, query: &GalleryQuery) -> Vec<String> {
        page(root, query).unwrap().items.into_iter().map(|item| item.name).collect()
    }

    #[test]
    fn filters_sorts_and_pages() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join(".scrappy")).unwrap();
        fs::create_dir_all(root.join("Images/a")).unwrap();
        fs::write(root.join("Images/a/zebra.jpg"), "x").unwrap();
        add(root, "Images/a/zebra.jpg", "image", "a", 300);
        add(root, "Videos/b/Apple 100%.mp4", "video", "b", 900);
        add(root, "Videos/b/mango_1.mp4", "video", "b", 100);

        assert_eq!(names(root, &query(GallerySort::Newest)), ["mango_1.mp4", "Apple 100%.mp4", "zebra.jpg"]);
        assert_eq!(names(root, &query(GallerySort::Largest))[0], "Apple 100%.mp4");
        assert_eq!(names(root, &query(GallerySort::Name)), ["Apple 100%.mp4", "mango_1.mp4", "zebra.jpg"]);

        let videos = GalleryQuery { kind: Some("video".into()), ..query(GallerySort::Oldest) };
        assert_eq!(names(root, &videos), ["Apple 100%.mp4", "mango_1.mp4"]);
        let percent = GalleryQuery { search: Some("100%".into()), ..query(GallerySort::Newest) };
        assert_eq!(names(root, &percent), ["Apple 100%.mp4"]);
        let underscore = GalleryQuery { search: Some("o_".into()), ..query(GallerySort::Newest) };
        assert_eq!(names(root, &underscore), ["mango_1.mp4"]);

        let second = GalleryQuery { offset: 1, limit: 1, ..query(GallerySort::Newest) };
        let found = page(root, &second).unwrap();
        assert_eq!((found.total, found.items.len(), found.items[0].name.as_str()), (3, 1, "Apple 100%.mp4"));
        let all = page(root, &query(GallerySort::Name)).unwrap().items;
        assert_eq!(all.iter().map(|item| item.missing).collect::<Vec<_>>(), [true, true, false]);
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

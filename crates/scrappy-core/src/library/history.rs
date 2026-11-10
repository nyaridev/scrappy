//! History: what the catalog says has been downloaded: totals, per kind, per site, per day, the latest files, and
//! finding one of them on disk.

// ── Imports ───────────────────────────────
use std::collections::HashSet;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, params};
use serde::Serialize;
use ts_rs::TS;

use super::catalog;

// ── Constants ─────────────────────────────
/// How many days the per-day counts cover, today included.
const DAYS: i64 = 30;

// ── Types ─────────────────────────────────
/// Files and bytes under one name: a kind (`video`) or a site (`youtube`).
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Tally {
    pub name: String,
    pub files: u32,
    #[ts(type = "number")]
    pub bytes: u64,
}

/// One calendar day in local time.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Day {
    /// `YYYY-MM-DD`.
    pub day: String,
    pub files: u32,
    #[ts(type = "number")]
    pub bytes: u64,
}

/// The library's numbers for Home.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Stats {
    pub files: u32,
    #[ts(type = "number")]
    pub bytes: u64,
    /// Most bytes first.
    pub kinds: Vec<Tally>,
    /// Most files first; downloads with no site aren't counted.
    pub sites: Vec<Tally>,
    /// The last 30 days, oldest first, days with nothing included.
    pub days: Vec<Day>,
}

/// A downloaded file, for Home's recent list.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RecentDownload {
    /// Relative to the library root, with `/` separators; also what `locate` takes.
    pub path: String,
    /// The file name without its extension.
    pub name: String,
    pub kind: String,
    pub site: Option<String>,
    #[ts(type = "number")]
    pub bytes: u64,
    /// Unix seconds.
    #[ts(type = "number")]
    pub downloaded_at: i64,
}

// ── Private helpers ───────────────────────
fn error(e: rusqlite::Error) -> String {
    format!("Can't read the library catalog: {e}")
}

fn unsigned(value: i64) -> u64 {
    u64::try_from(value).unwrap_or(0)
}

fn count(value: i64) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

fn tallies(connection: &Connection, column: &str, order: &str) -> Result<Vec<Tally>, String> {
    // `column` and `order` are this file's constants, never input.
    let sql = format!(
        "SELECT {column}, COUNT(*), TOTAL(size_bytes) FROM downloads WHERE {column} IS NOT NULL
         GROUP BY {column} ORDER BY {order} DESC, {column}"
    );
    let mut statement = connection.prepare(&sql).map_err(error)?;
    let rows = statement
        .query_map([], |row| {
            Ok(Tally { name: row.get(0)?, files: count(row.get(1)?), bytes: row.get::<_, f64>(2)? as u64 })
        })
        .map_err(error)?;
    rows.collect::<Result<_, _>>().map_err(error)
}

fn days(connection: &Connection) -> Result<Vec<Day>, String> {
    // The recursive part lists every day, so days with no downloads still get a row.
    let mut statement = connection
        .prepare(
            "WITH RECURSIVE span(day) AS (
                 SELECT date('now', 'localtime', ?1)
                 UNION ALL SELECT date(day, '+1 day') FROM span WHERE day < date('now', 'localtime')
             )
             SELECT span.day, COUNT(downloads.id), TOTAL(downloads.size_bytes) FROM span
             LEFT JOIN downloads ON date(downloads.downloaded_at, 'unixepoch', 'localtime') = span.day
             GROUP BY span.day ORDER BY span.day",
        )
        .map_err(error)?;
    let rows = statement
        .query_map(params![format!("-{} days", DAYS - 1)], |row| {
            Ok(Day { day: row.get(0)?, files: count(row.get(1)?), bytes: row.get::<_, f64>(2)? as u64 })
        })
        .map_err(error)?;
    rows.collect::<Result<_, _>>().map_err(error)
}

// ── Public API ────────────────────────────
/// The totals, the per-kind and per-site tallies and the last 30 days of the library at `root`.
pub fn stats(root: &Path) -> Result<Stats, String> {
    let connection = catalog::open(root)?;
    let (files, bytes): (i64, f64) = connection
        .query_row("SELECT COUNT(*), TOTAL(size_bytes) FROM downloads", [], |row| Ok((row.get(0)?, row.get(1)?)))
        .map_err(error)?;
    Ok(Stats {
        files: count(files),
        bytes: bytes as u64,
        kinds: tallies(&connection, "kind", "TOTAL(size_bytes)")?,
        sites: tallies(&connection, "site", "COUNT(*)")?,
        days: days(&connection)?,
    })
}

/// The `limit` latest downloads, newest first.
pub fn recent(root: &Path, limit: u32) -> Result<Vec<RecentDownload>, String> {
    let connection = catalog::open(root)?;
    let mut statement = connection
        .prepare(
            "SELECT path, kind, site, size_bytes, downloaded_at FROM downloads
             ORDER BY downloaded_at DESC, id DESC LIMIT ?1",
        )
        .map_err(error)?;
    let rows = statement
        .query_map(params![limit], |row| {
            let path: String = row.get(0)?;
            let name = Path::new(&path).file_stem().map_or_else(|| path.clone(), |s| s.to_string_lossy().into());
            Ok(RecentDownload {
                path,
                name,
                kind: row.get(1)?,
                site: row.get(2)?,
                bytes: unsigned(row.get(3)?),
                downloaded_at: row.get(4)?,
            })
        })
        .map_err(error)?;
    rows.collect::<Result<_, _>>().map_err(error)
}

/// Every link the catalog has saved a file from.
pub fn sources(root: &Path) -> Result<HashSet<String>, String> {
    let connection = catalog::open(root)?;
    let mut statement = connection.prepare("SELECT source_url FROM downloads WHERE source_url IS NOT NULL").map_err(error)?;
    let rows = statement.query_map([], |row| row.get(0)).map_err(error)?;
    rows.collect::<Result<_, _>>().map_err(error)
}

/// Today's local date, `YYYY-MM-DD`, as the catalog counts days.
pub fn today() -> Result<String, String> {
    // SQLite already knows the local time zone, so one date needs no date crate.
    let connection = Connection::open_in_memory().map_err(error)?;
    connection.query_row("SELECT date('now', 'localtime')", [], |row| row.get(0)).map_err(error)
}

/// The full path of a file in the library, from its catalog path. Refuses anything that leads outside `root` or
/// doesn't exist.
pub fn locate(root: &Path, relative: &str) -> Result<PathBuf, String> {
    let real_root = root.canonicalize().map_err(|_| "The library folder is missing.".to_string())?;
    let real_file =
        root.join(relative).canonicalize().map_err(|_| "This file is no longer in the library.".to_string())?;
    if real_file == real_root || !real_file.starts_with(&real_root) {
        return Err("This file isn't in the library.".into());
    }
    // Windows gives canonical paths a `\\?\` prefix that file managers don't all take.
    Ok(root.join(relative))
}

#[cfg(test)]
mod tests {
    use super::{locate, recent, sources, stats, today};
    use crate::library::catalog::{self, Record};
    use std::fs;

    fn library() -> tempfile::TempDir {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir(root.path().join(".scrappy")).unwrap();
        root
    }

    fn add(root: &std::path::Path, path: &str, kind: &'static str, site: Option<&str>, size: u64) {
        let row = Record {
            path: path.into(),
            kind,
            source_url: "https://example.com".into(),
            site: site.map(Into::into),
            user: None,
            size_bytes: size,
            duration_secs: None,
            codec: None,
            transferred_bytes: size,
        };
        catalog::record(root, &row).unwrap();
    }

    #[test]
    fn counts_an_empty_library() {
        let root = library();
        let numbers = stats(root.path()).unwrap();
        assert_eq!((numbers.files, numbers.bytes), (0, 0));
        assert!(numbers.kinds.is_empty() && numbers.sites.is_empty());
        assert_eq!(numbers.days.len(), 30);
        assert!(numbers.days.windows(2).all(|pair| pair[0].day < pair[1].day));
        assert!(recent(root.path(), 10).unwrap().is_empty());
    }

    #[test]
    fn tallies_kinds_sites_and_today() {
        let root = library();
        add(root.path(), "Videos/a/One [1].mp4", "video", Some("youtube"), 100);
        add(root.path(), "Videos/a/Two [2].mp4", "video", Some("youtube"), 300);
        add(root.path(), "Audio/b/Three [3].mp3", "audio", Some("soundcloud"), 50);
        add(root.path(), "Audio/b/Four.mp3", "audio", None, 1000);

        let numbers = stats(root.path()).unwrap();
        assert_eq!((numbers.files, numbers.bytes), (4, 1450));
        let kinds: Vec<_> = numbers.kinds.iter().map(|t| (t.name.as_str(), t.files, t.bytes)).collect();
        assert_eq!(kinds, [("audio", 2, 1050), ("video", 2, 400)]);
        let sites: Vec<_> = numbers.sites.iter().map(|t| (t.name.as_str(), t.files)).collect();
        assert_eq!(sites, [("youtube", 2), ("soundcloud", 1)]);
        let today = numbers.days.last().unwrap();
        assert_eq!((today.files, today.bytes), (4, 1450));

        assert_eq!(sources(root.path()).unwrap().len(), 1);
        let latest = recent(root.path(), 2).unwrap();
        assert_eq!(latest.len(), 2);
        assert_eq!(latest[0].name, "Four");
        assert_eq!(latest[1].path, "Audio/b/Three [3].mp3");
    }

    #[test]
    fn knows_todays_date() {
        let day = today().unwrap();
        assert_eq!((day.len(), &day[4..5], &day[7..8]), (10, "-", "-"));
        assert_eq!(stats(library().path()).unwrap().days.last().unwrap().day, day);
    }

    #[test]
    fn locates_only_files_inside_the_library() {
        let root = library();
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("secret.txt"), "x").unwrap();
        fs::create_dir_all(root.path().join("Videos/a")).unwrap();
        fs::write(root.path().join("Videos/a/One.mp4"), "x").unwrap();

        let found = locate(root.path(), "Videos/a/One.mp4").unwrap();
        assert!(found.ends_with("One.mp4"));
        assert!(locate(root.path(), "Videos/a/Gone.mp4").unwrap_err().contains("no longer"));
        assert!(locate(root.path(), "").is_err());
        let escape = format!("../{}/secret.txt", outside.path().file_name().unwrap().to_string_lossy());
        assert!(locate(root.path(), &escape).unwrap_err().contains("isn't in"));
        let absolute = outside.path().join("secret.txt");
        assert!(locate(root.path(), &absolute.to_string_lossy()).unwrap_err().contains("isn't in"));
    }
}

//! Catalog: the SQLite database at `<root>/.scrappy/library.db`, one row per downloaded file, and its numbered
//! migrations (ADR-0004).

// ── Imports ───────────────────────────────
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, params};

// ── Constants ─────────────────────────────
/// The catalog's place inside the library root.
pub(super) const FILE: &str = ".scrappy/library.db";

/// Entry `n` moves the schema from version `n` to `n + 1`. Only ever append: shipped entries never change.
const MIGRATIONS: &[&str] = &["
    CREATE TABLE downloads (
        id                INTEGER PRIMARY KEY,
        path              TEXT NOT NULL UNIQUE, -- relative to the library root
        kind              TEXT NOT NULL,        -- video, audio, social, image, file, site
        source_url        TEXT,
        site              TEXT,
        size_bytes        INTEGER NOT NULL DEFAULT 0,
        duration_secs     REAL,
        codec             TEXT,
        transferred_bytes INTEGER NOT NULL DEFAULT 0,
        downloaded_at     INTEGER NOT NULL      -- unix seconds
    );
    CREATE INDEX downloads_by_time ON downloads (downloaded_at);
", "
    ALTER TABLE downloads ADD COLUMN user TEXT; -- the account or channel it came from
", "
    -- An image's perceptual hash and size, for duplicate removal; empty until it's hashed. A row replaced by a new
    -- download starts empty again.
    ALTER TABLE downloads ADD COLUMN phash INTEGER;
    ALTER TABLE downloads ADD COLUMN width INTEGER;
    ALTER TABLE downloads ADD COLUMN height INTEGER;
"];

// ── Types ─────────────────────────────────
/// A finished download, as the catalog keeps it.
#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    /// Relative to the library root, with `/` separators.
    pub path: String,
    /// `video`, `audio`, `image`, `social` or `file`; one per download kind (ADR-0004).
    pub kind: &'static str,
    pub source_url: String,
    pub site: Option<String>,
    /// The account or channel it came from.
    pub user: Option<String>,
    pub size_bytes: u64,
    pub duration_secs: Option<f64>,
    pub codec: Option<String>,
    pub transferred_bytes: u64,
}

// ── Private helpers ───────────────────────
fn path(root: &Path) -> PathBuf {
    root.join(FILE)
}

fn migrate(connection: &mut Connection) -> Result<(), String> {
    let error = |e: rusqlite::Error| format!("Can't update the library catalog: {e}");
    let version: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0)).map_err(error)?;
    let applied = usize::try_from(version).unwrap_or(usize::MAX);
    if applied > MIGRATIONS.len() {
        return Err("This library was made by a newer Scrappy. Update Scrappy to open it.".into());
    }
    for (index, sql) in MIGRATIONS.iter().enumerate().skip(applied) {
        let transaction = connection.transaction().map_err(error)?;
        transaction.execute_batch(sql).map_err(error)?;
        transaction.pragma_update(None, "user_version", (index + 1) as i64).map_err(error)?;
        transaction.commit().map_err(error)?;
    }
    Ok(())
}

// ── Public API ────────────────────────────
/// Whether `root` already holds a catalog.
pub(super) fn exists(root: &Path) -> bool {
    path(root).is_file()
}

/// Opens the catalog in `root` (its `.scrappy` folder must exist), creating it or bringing its schema up to date.
pub(super) fn open(root: &Path) -> Result<Connection, String> {
    let mut connection = Connection::open(path(root)).map_err(|e| format!("Can't open the library catalog: {e}"))?;
    migrate(&mut connection)?;
    Ok(connection)
}

/// Adds a finished download, or replaces the row for the same path (a download done again).
pub fn record(root: &Path, record: &Record) -> Result<(), String> {
    // SQLite integers are i64; no file or clock gets near its limit.
    let signed = |value: u64| i64::try_from(value).unwrap_or(i64::MAX);
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |since| signed(since.as_secs()));
    open(root)?
        .execute(
            "INSERT OR REPLACE INTO downloads (path, kind, source_url, site, size_bytes, duration_secs, codec,
                transferred_bytes, downloaded_at, user) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                record.path,
                record.kind,
                record.source_url,
                record.site,
                signed(record.size_bytes),
                record.duration_secs,
                record.codec,
                signed(record.transferred_bytes),
                now,
                record.user
            ],
        )
        .map_err(|e| format!("Can't add the download to the library catalog: {e}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{MIGRATIONS, Record, exists, open, record};
    use std::fs;

    fn version(connection: &rusqlite::Connection) -> usize {
        let version: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0)).unwrap();
        usize::try_from(version).unwrap()
    }

    #[test]
    fn creates_the_schema_once() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir(root.path().join(".scrappy")).unwrap();
        assert!(!exists(root.path()));
        assert_eq!(version(&open(root.path()).unwrap()), MIGRATIONS.len());
        assert!(exists(root.path()));
        let again = open(root.path()).unwrap();
        assert_eq!(version(&again), MIGRATIONS.len());
        again.execute("INSERT INTO downloads (path, kind, downloaded_at) VALUES ('a.mp4', 'video', 0)", []).unwrap();
    }

    #[test]
    fn records_a_download_once_per_path() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir(root.path().join(".scrappy")).unwrap();
        let mut row = Record {
            path: "Videos/jawed/Me at the zoo [jNQXAC9IVRw].mp4".into(),
            kind: "video",
            source_url: "https://www.youtube.com/watch?v=jNQXAC9IVRw".into(),
            site: Some("youtube".into()),
            user: Some("jawed".into()),
            size_bytes: 629_172,
            duration_secs: Some(18.9),
            codec: Some("h264".into()),
            transferred_bytes: 629_172,
        };
        record(root.path(), &row).unwrap();
        row.size_bytes = 1;
        record(root.path(), &row).unwrap();
        let rows: Vec<i64> = open(root.path())
            .unwrap()
            .prepare("SELECT size_bytes FROM downloads")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        assert_eq!(rows, vec![1]);
    }

    #[test]
    fn refuses_a_newer_catalog() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir(root.path().join(".scrappy")).unwrap();
        open(root.path()).unwrap().pragma_update(None, "user_version", MIGRATIONS.len() as i64 + 1).unwrap();
        assert!(open(root.path()).unwrap_err().contains("newer Scrappy"));
    }
}

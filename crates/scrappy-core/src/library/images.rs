//! Images: the catalog's picture files for duplicate removal, the perceptual hashes kept with them, and forgetting
//! files that left the library (ADR-0004, ADR-0009).

// ── Imports ───────────────────────────────
use std::path::Path;

use rusqlite::params;

use super::catalog;

// ── Constants ─────────────────────────────
/// Picture types duplicate removal reads, by extension.
const PICTURES: &[&str] = &["jpg", "jpeg", "png", "webp", "gif", "bmp", "tif", "tiff"];

// ── Types ─────────────────────────────────
/// A picture in the catalog, with its hash once it has one.
#[derive(Debug, Clone, PartialEq)]
pub struct ImageRow {
    /// Relative to the library root, with `/` separators.
    pub path: String,
    pub bytes: u64,
    /// The perceptual hash, width and height, once hashed.
    pub fingerprint: Option<(u64, u32, u32)>,
}

// ── Private helpers ───────────────────────
fn error(e: rusqlite::Error) -> String {
    format!("Can't read the library catalog: {e}")
}

fn is_picture(path: &str) -> bool {
    let ext = path.rsplit_once('.').map(|(_, ext)| ext.to_ascii_lowercase()).unwrap_or_default();
    PICTURES.contains(&ext.as_str())
}

// ── Public API ────────────────────────────
/// The pictures in the catalog, all of them or those under `folder` (relative, `/`-separated).
pub fn images(root: &Path, folder: Option<&str>) -> Result<Vec<ImageRow>, String> {
    let connection = catalog::open(root)?;
    let prefix = folder.map(|folder| format!("{}/", folder.trim_matches('/')));
    let mut statement =
        connection.prepare("SELECT path, size_bytes, phash, width, height FROM downloads ORDER BY id").map_err(error)?;
    let rows = statement
        .query_map([], |row| {
            let (hash, width, height): (Option<i64>, Option<u32>, Option<u32>) = (row.get(2)?, row.get(3)?, row.get(4)?);
            Ok(ImageRow {
                path: row.get(0)?,
                bytes: u64::try_from(row.get::<_, i64>(1)?).unwrap_or(0),
                // Stored as SQLite's signed integer; the bits are the hash.
                fingerprint: hash.zip(width).zip(height).map(|((hash, width), height)| (hash as u64, width, height)),
            })
        })
        .map_err(error)?;
    let rows: Vec<ImageRow> = rows.collect::<Result<_, _>>().map_err(error)?;
    Ok(rows
        .into_iter()
        .filter(|row| is_picture(&row.path))
        .filter(|row| prefix.as_deref().is_none_or(|prefix| row.path.starts_with(prefix)))
        .collect())
}

/// Keeps each picture's perceptual hash, width and height with its catalog row, all in one transaction.
pub fn set_fingerprints(root: &Path, made: &[(String, (u64, u32, u32))]) -> Result<(), String> {
    let failed = |e: rusqlite::Error| format!("Can't update the library catalog: {e}");
    let mut connection = catalog::open(root)?;
    let transaction = connection.transaction().map_err(failed)?;
    for (path, (hash, width, height)) in made {
        transaction
            .execute(
                "UPDATE downloads SET phash = ?1, width = ?2, height = ?3 WHERE path = ?4",
                params![*hash as i64, width, height, path],
            )
            .map_err(failed)?;
    }
    transaction.commit().map_err(failed)
}

/// Drops the catalog rows of files that left the library; the files themselves aren't touched.
pub fn forget(root: &Path, paths: &[String]) -> Result<(), String> {
    let mut connection = catalog::open(root)?;
    let transaction = connection.transaction().map_err(|e| format!("Can't update the library catalog: {e}"))?;
    for path in paths {
        transaction
            .execute("DELETE FROM downloads WHERE path = ?1", params![path])
            .map_err(|e| format!("Can't update the library catalog: {e}"))?;
    }
    transaction.commit().map_err(|e| format!("Can't update the library catalog: {e}"))
}

#[cfg(test)]
mod tests {
    use super::{forget, images, set_fingerprints};
    use crate::library::catalog::{self, Record};
    use std::fs;

    fn add(root: &std::path::Path, path: &str) {
        let record = Record {
            path: path.into(),
            kind: "image",
            source_url: "https://example.com".into(),
            site: None,
            user: None,
            size_bytes: 10,
            duration_secs: None,
            codec: None,
            transferred_bytes: 10,
        };
        catalog::record(root, &record).unwrap();
    }

    #[test]
    fn keeps_hashes_and_forgets_rows() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join(".scrappy")).unwrap();
        for path in ["Images/a/one.JPG", "Images/b/two.png", "Videos/c.mp4", "Images/ab/three.webp"] {
            add(root, path);
        }
        let paths = |rows: Vec<super::ImageRow>| rows.into_iter().map(|row| row.path).collect::<Vec<_>>();
        assert_eq!(paths(images(root, None).unwrap()), ["Images/a/one.JPG", "Images/b/two.png", "Images/ab/three.webp"]);
        assert_eq!(paths(images(root, Some("Images/a")).unwrap()), ["Images/a/one.JPG"]);

        // The top bit set: it must survive SQLite's signed integers.
        set_fingerprints(root, &[("Images/b/two.png".into(), (u64::MAX - 5, 640, 480))]).unwrap();
        let two = images(root, Some("Images/b")).unwrap().remove(0);
        assert_eq!(two.fingerprint, Some((u64::MAX - 5, 640, 480)));
        // Downloaded again: the row starts without a hash.
        add(root, "Images/b/two.png");
        assert_eq!(images(root, Some("Images/b")).unwrap()[0].fingerprint, None);

        forget(root, &["Images/a/one.JPG".to_string()]).unwrap();
        assert_eq!(images(root, None).unwrap().len(), 2);
    }
}

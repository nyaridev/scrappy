//! Remove: deleting a library file, or a downloaded folder with everything in it, for good, and dropping them from
//! the catalog (ADR-0022). The library's own folders and `.scrappy` are never deleted.

// ── Imports ───────────────────────────────
use std::fs;
use std::path::Path;

use rusqlite::params;

use super::catalog;
use super::history::locate;

// ── Constants ─────────────────────────────
/// The app's own folder: the catalog and thumbnails.
const APP_FOLDER: &str = ".scrappy";

// ── Private helpers ───────────────────────
/// Drops `path`'s row, or every row inside it when it's a folder.
fn forget(root: &Path, path: &str) -> Result<(), String> {
    let connection = catalog::open(root)?;
    connection
        .execute(
            "DELETE FROM downloads WHERE path = ?1 OR substr(path, 1, length(?2)) = ?2",
            params![path, format!("{path}/")],
        )
        .map_err(|e| format!("Can't update the library catalog: {e}"))?;
    Ok(())
}

// ── Public API ────────────────────────────
/// Deletes the library file or folder at `path` (relative to `root`, `/` separators) for good. A folder must sit
/// below the top level (a channel's, an account's), and nothing in `.scrappy` can go.
pub fn remove(root: &Path, path: &str) -> Result<(), String> {
    let path = path.trim_matches('/');
    if path.split('/').next() == Some(APP_FOLDER) {
        return Err("Scrappy's own folder can't be deleted.".into());
    }
    let full = locate(root, path)?;
    // Not followed: a link is deleted as the link, never what it points to.
    let info = fs::symlink_metadata(&full).map_err(|e| format!("Can't read {path}: {e}"))?;
    if info.is_dir() {
        if !path.contains('/') {
            return Err("The library's own folders can't be deleted.".into());
        }
        fs::remove_dir_all(&full).map_err(|e| format!("Can't delete {path}: {e}"))?;
    } else {
        fs::remove_file(&full).map_err(|e| format!("Can't delete {path}: {e}"))?;
    }
    forget(root, path)?;
    tracing::info!("Deleted a library {}", if info.is_dir() { "folder" } else { "file" });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::remove;
    use crate::library::catalog::{self, Record};
    use std::fs;

    fn add(root: &std::path::Path, path: &str) {
        fs::create_dir_all(root.join(path).parent().unwrap()).unwrap();
        fs::write(root.join(path), "x").unwrap();
        let record = Record {
            path: path.into(),
            kind: "video",
            source_url: "https://example.com".into(),
            site: None,
            user: None,
            size_bytes: 1,
            duration_secs: None,
            codec: None,
            transferred_bytes: 1,
        };
        catalog::record(root, &record).unwrap();
    }

    fn rows(root: &std::path::Path) -> Vec<String> {
        let connection = catalog::open(root).unwrap();
        let mut statement = connection.prepare("SELECT path FROM downloads ORDER BY path").unwrap();
        statement.query_map([], |row| row.get(0)).unwrap().map(Result::unwrap).collect()
    }

    #[test]
    fn deletes_a_file_or_a_downloaded_folder_and_nothing_else() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join(".scrappy/thumbs")).unwrap();
        add(root, "Videos/NASA/a.mp4");
        add(root, "Videos/NASA/old/b.mp4");
        add(root, "Videos/NASA b/c.mp4");
        add(root, "Videos/other.mp4");

        remove(root, "Videos/other.mp4").unwrap();
        assert!(!root.join("Videos/other.mp4").exists());
        remove(root, "Videos/NASA").unwrap();
        assert!(!root.join("Videos/NASA").exists());
        // A folder whose name only starts the same stays, file and row.
        assert!(root.join("Videos/NASA b/c.mp4").is_file());
        assert_eq!(rows(root), ["Videos/NASA b/c.mp4"]);

        assert!(remove(root, "Videos").unwrap_err().contains("own folders"));
        assert!(remove(root, ".scrappy").unwrap_err().contains("own folder"));
        assert!(remove(root, ".scrappy/thumbs").unwrap_err().contains("own folder"));
        assert!(remove(root, "../outside").is_err());
        assert!(remove(root, "").is_err());
        assert!(remove(root, "Videos/gone.mp4").is_err());
        assert!(root.join("Videos").is_dir() && root.join(".scrappy/thumbs").is_dir());
    }
}

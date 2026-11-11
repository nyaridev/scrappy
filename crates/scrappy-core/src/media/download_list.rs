//! Download list: the Download list kept on disk (`downloads.db` in the app's data folder, ADR-0016), one row per
//! download as JSON. A row fails to read or write only into the log; downloads go on without it.

// ── Imports ───────────────────────────────
use std::path::Path;
use std::sync::{Arc, Mutex, PoisonError};

use rusqlite::{Connection, params};

use super::downloads::{DownloadEntry, DownloadState};
use crate::jobs::JobId;

// ── Constants ─────────────────────────────
const FILE: &str = "downloads.db";

// ── Types ─────────────────────────────────
/// The open list; `None` when it couldn't be opened (the list then lasts the session).
#[derive(Clone, Default)]
pub(super) struct DownloadList(Option<Arc<Mutex<Connection>>>);

// ── Private helpers ───────────────────────
fn open(dir: &Path) -> Result<Connection, String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let connection = Connection::open(dir.join(FILE)).map_err(|e| e.to_string())?;
    // A write-ahead log, synced at checkpoints: a row is written in microseconds instead of waiting for the disk, so
    // queueing a thousand grabs doesn't hold up the list. The file is in the app's own data folder, never a share.
    connection
        .execute_batch(
            "PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL;
             CREATE TABLE IF NOT EXISTS downloads (id INTEGER PRIMARY KEY, entry TEXT NOT NULL)",
        )
        .map_err(|e| e.to_string())?;
    Ok(connection)
}

// ── Public API ────────────────────────────
impl DownloadList {
    /// Opens the list in `dir`, creating it when it's missing.
    pub(super) fn open(dir: &Path) -> Self {
        match open(dir) {
            Ok(connection) => Self(Some(Arc::new(Mutex::new(connection)))),
            Err(error) => {
                tracing::warn!("The download list can't be opened: {error}");
                Self(None)
            }
        }
    }

    /// The kept downloads, oldest first. One queued or running when Scrappy closed is stopped now.
    pub(super) fn load(&self) -> Vec<DownloadEntry> {
        let Some(connection) = &self.0 else { return Vec::new() };
        let rows: Vec<String> = {
            let connection = connection.lock().unwrap_or_else(PoisonError::into_inner);
            let read = connection.prepare("SELECT entry FROM downloads ORDER BY id").and_then(|mut statement| {
                statement.query_map([], |row| row.get(0))?.collect::<rusqlite::Result<Vec<String>>>()
            });
            read.unwrap_or_else(|error| {
                tracing::warn!("The download list can't be read: {error}");
                Vec::new()
            })
        };
        let mut entries: Vec<DownloadEntry> = rows.iter().filter_map(|row| serde_json::from_str(row).ok()).collect();
        for entry in &mut entries {
            if matches!(entry.state, DownloadState::Queued | DownloadState::Running) {
                entry.state = DownloadState::Cancelled;
                self.put(entry);
            }
        }
        entries
    }

    /// Adds or replaces a download's row.
    pub(super) fn put(&self, entry: &DownloadEntry) {
        let Some(connection) = &self.0 else { return };
        let Ok(json) = serde_json::to_string(entry) else { return };
        let connection = connection.lock().unwrap_or_else(PoisonError::into_inner);
        if let Err(error) =
            connection.execute("INSERT OR REPLACE INTO downloads (id, entry) VALUES (?1, ?2)", params![entry.id, json])
        {
            tracing::warn!("A download can't be kept: {error}");
        }
    }

    /// Removes the rows of `ids`.
    pub(super) fn remove(&self, ids: &[JobId]) {
        let Some(connection) = &self.0 else { return };
        let connection = connection.lock().unwrap_or_else(PoisonError::into_inner);
        for id in ids {
            if let Err(error) = connection.execute("DELETE FROM downloads WHERE id = ?1", params![id]) {
                tracing::warn!("A download can't be removed from the list: {error}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::DownloadList;
    use crate::media::{DownloadEntry, DownloadState};

    fn entry(id: u32, state: DownloadState) -> DownloadEntry {
        DownloadEntry {
            id,
            url: "https://example.com/v".into(),
            title: format!("Video {id}"),
            preset: "MP4".into(),
            state,
            thumbnail: None,
            file: None,
            files: None,
            skipped: None,
            error: None,
        }
    }

    #[test]
    fn keeps_downloads_across_opens_and_stops_unfinished_ones() {
        let dir = tempfile::tempdir().unwrap();
        let list = DownloadList::open(dir.path());
        list.put(&entry(1, DownloadState::Done));
        list.put(&entry(2, DownloadState::Running));
        list.put(&entry(3, DownloadState::Failed));
        list.remove(&[3]);

        let again = DownloadList::open(dir.path()).load();
        let states: Vec<_> = again.iter().map(|entry| (entry.id, entry.state)).collect();
        assert_eq!(states, [(1, DownloadState::Done), (2, DownloadState::Cancelled)]);
        assert_eq!(DownloadList::open(dir.path()).load()[1].state, DownloadState::Cancelled);
    }
}

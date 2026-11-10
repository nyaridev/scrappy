//! Library folder: checking a folder the user picked or typed, creating the layout in it, and where the library
//! stands (ADR-0004).

// ── Imports ───────────────────────────────
use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;
use ts_rs::TS;

use super::catalog;
use crate::settings;

// ── Constants ─────────────────────────────
/// The folders setup creates in the root. `.scrappy` holds the catalog.
const LAYOUT: [&str; 7] = ["Videos", "Audio", "Social", "Grabs", "Images", "Sites", ".scrappy"];
const WRITE_PROBE: &str = ".scrappy-write-test";

// ── Types ─────────────────────────────────
/// What Scrappy found at a folder path the user picked or typed.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FolderCheck {
    /// The path as Scrappy will use it.
    pub path: PathBuf,
    /// It's there already; otherwise setup creates it.
    pub exists: bool,
    /// Scrappy can create files in it, or create it.
    pub writable: bool,
    /// It holds nothing yet.
    pub empty: bool,
    /// It's already a Scrappy library, so setup reuses its catalog.
    pub library: bool,
}

/// Where the library stands, for the first-run screen.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(tag = "state", rename_all = "camelCase")]
#[ts(export)]
pub enum LibraryStatus {
    /// No folder picked yet.
    NotSet,
    /// The folder and its catalog are there.
    Ready { root: PathBuf },
    /// The folder or its catalog is gone (an unplugged drive, a deleted folder).
    Missing { root: PathBuf },
}

// ── Private helpers ───────────────────────
/// Turns what the user typed or picked into a full folder path. Relative paths are refused: they'd depend on where
/// Scrappy happened to start.
fn resolve(input: &str) -> Result<PathBuf, String> {
    let path = Path::new(input.trim());
    if path.as_os_str().is_empty() {
        return Err("Enter a folder.".into());
    }
    if !path.is_absolute() {
        return Err("Not a full path.".into());
    }
    Ok(path.components().collect())
}

fn writable(path: &Path) -> bool {
    let probe = path.join(WRITE_PROBE);
    fs::write(&probe, b"").and_then(|()| fs::remove_file(&probe)).is_ok()
}

// ── Public API ────────────────────────────
/// Checks a folder path before it becomes the library. A folder that isn't there yet is fine when its nearest
/// existing parent is writable.
pub fn check(input: &str) -> Result<FolderCheck, String> {
    let path = resolve(input)?;
    let exists = path.exists();
    let writable = if exists {
        path.is_dir() && writable(&path)
    } else {
        path.ancestors().find(|parent| parent.exists()).is_some_and(|parent| parent.is_dir() && writable(parent))
    };
    Ok(FolderCheck {
        exists,
        writable,
        empty: !exists || fs::read_dir(&path).is_ok_and(|mut entries| entries.next().is_none()),
        library: catalog::exists(&path),
        path,
    })
}

/// Where the library in `root` (from the settings) stands.
pub fn status(root: Option<&Path>) -> LibraryStatus {
    match root {
        None => LibraryStatus::NotSet,
        Some(root) if catalog::exists(root) => LibraryStatus::Ready { root: root.to_path_buf() },
        Some(root) => LibraryStatus::Missing { root: root.to_path_buf() },
    }
}

/// Creates the folder if needed, the layout and the catalog in it, keeping anything already there, and saves it as
/// the library folder.
pub fn setup(input: &str, settings: &settings::Store) -> Result<LibraryStatus, String> {
    let found = check(input)?;
    if !found.writable {
        return Err("Can't use this folder.".into());
    }
    let root = found.path.as_path();
    for folder in LAYOUT {
        fs::create_dir_all(root.join(folder)).map_err(|e| format!("Can't create the {folder} folder: {e}"))?;
    }
    catalog::open(root)?;
    settings.set_library_root(root.to_path_buf())?;
    tracing::info!("Library set up");
    Ok(LibraryStatus::Ready { root: root.to_path_buf() })
}

#[cfg(test)]
mod tests {
    use super::{LAYOUT, LibraryStatus, check, setup, status};
    use crate::settings::Store;
    use std::fs;
    use std::path::Path;

    fn text(path: &Path) -> &str {
        path.to_str().unwrap()
    }

    #[test]
    fn checks_a_folder() {
        let root = tempfile::tempdir().unwrap();
        let fresh = check(text(root.path())).unwrap();
        assert!(fresh.exists && fresh.writable && fresh.empty && !fresh.library);
        fs::write(root.path().join("old.txt"), "keep me").unwrap();
        assert!(!check(text(root.path())).unwrap().empty);
        assert!(!check(text(&root.path().join("old.txt"))).unwrap().writable);
    }

    #[test]
    fn accepts_a_folder_that_isnt_there_yet() {
        let root = tempfile::tempdir().unwrap();
        let new = check(text(&root.path().join("New").join("Library"))).unwrap();
        assert!(!new.exists && new.writable && new.empty);
    }

    #[test]
    fn refuses_empty_and_relative_paths() {
        assert_eq!(check("  ").unwrap_err(), "Enter a folder.");
        assert_eq!(check("Scrappy").unwrap_err(), "Not a full path.");
        assert_eq!(check("../Scrappy").unwrap_err(), "Not a full path.");
    }

    #[test]
    fn sets_up_and_keeps_what_was_there() {
        let root = tempfile::tempdir().unwrap();
        let config = tempfile::tempdir().unwrap();
        let settings = Store::load(config.path());
        fs::write(root.path().join("old.txt"), "keep me").unwrap();

        assert_eq!(status(None), LibraryStatus::NotSet);
        assert_eq!(status(Some(root.path())), LibraryStatus::Missing { root: root.path().to_path_buf() });
        let ready = setup(&format!(" {} ", text(root.path())), &settings).unwrap();

        assert_eq!(status(Some(root.path())), ready);
        assert!(LAYOUT.iter().all(|folder| root.path().join(folder).is_dir()));
        assert_eq!(fs::read_to_string(root.path().join("old.txt")).unwrap(), "keep me");
        assert_eq!(settings.get().library_root.as_deref(), Some(root.path()));
        assert!(check(text(root.path())).unwrap().library);
        assert_eq!(setup(text(root.path()), &settings).unwrap(), ready);
    }

    #[test]
    fn creates_a_new_folder() {
        let root = tempfile::tempdir().unwrap();
        let settings = Store::load(&root.path().join("config"));
        let library = root.path().join("New").join("Library");
        setup(text(&library), &settings).unwrap();
        assert!(library.join(".scrappy").is_dir());
    }
}

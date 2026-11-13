//! Disk: the library folder as it is on disk, for the Gallery. Its files are listed (hidden folders, downloads in
//! progress and a mirror's own files left out), kept in memory, and listed again after a watcher sees a change, which
//! it also passes on (ADR-0015).

// ── Imports ───────────────────────────────
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError, Weak};
use std::time::{Duration, UNIX_EPOCH};

use notify_debouncer_mini::notify::{RecommendedWatcher, RecursiveMode};
use notify_debouncer_mini::{DebounceEventResult, Debouncer, new_debouncer};

// ── Constants ─────────────────────────────
/// Changes closer together than this are one change.
const QUIET: Duration = Duration::from_millis(500);
/// Extensions of downloads still in progress.
const PARTIAL: &[&str] = &["part", "ytdl", "tmp", "temp", "crdownload"];
const SITES: &str = "Sites";
const PAGES: &str = "_pages";
const START_PAGE: &str = "index.html";

// ── Types ─────────────────────────────────
/// A file in the library folder.
#[derive(Debug, Clone, PartialEq)]
pub struct DiskFile {
    /// Relative to the library root, with `/` separators.
    pub path: String,
    pub bytes: u64,
    /// Unix seconds.
    pub modified: i64,
}

/// Called when the library folder changes.
pub type OnChange = Arc<dyn Fn() + Send + Sync>;

#[derive(Default)]
struct State {
    root: Option<PathBuf>,
    files: Option<Arc<Vec<DiskFile>>>,
    watcher: Option<Debouncer<RecommendedWatcher>>,
}

/// The library folder's files, kept until it changes.
#[derive(Clone)]
pub struct Disk {
    state: Arc<Mutex<State>>,
    on_change: OnChange,
}

// ── Private helpers ───────────────────────
/// Left out of the Gallery: anything hidden (`.scrappy/`), a download in progress, and a mirror's files other than its
/// start page and saved pages.
fn left_out(relative: &Path) -> bool {
    let parts: Vec<String> = relative
        .components()
        .filter_map(|part| match part {
            Component::Normal(name) => Some(name.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect();
    let Some(last) = parts.last() else { return true };
    let partial = last.rsplit_once('.').is_some_and(|(_, ext)| PARTIAL.contains(&ext.to_ascii_lowercase().as_str()));
    if partial || parts.iter().any(|part| part.starts_with('.')) {
        return true;
    }
    match parts.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
        [SITES, _, START_PAGE] => false,
        [SITES, _, PAGES, page] => !page.to_ascii_lowercase().ends_with(".html"),
        [SITES, ..] => true,
        _ => false,
    }
}

/// Every file under `root` the Gallery shows. Linked folders aren't followed, so a link can't loop.
fn list(root: &Path) -> Result<Vec<DiskFile>, String> {
    std::fs::read_dir(root).map_err(|e| format!("Can't read the library folder: {e}"))?;
    let mut files = Vec::new();
    let mut folders = vec![root.to_path_buf()];
    while let Some(folder) = folders.pop() {
        let Ok(entries) = std::fs::read_dir(&folder) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(relative) = path.strip_prefix(root) else { continue };
            let Ok(kind) = entry.file_type() else { continue };
            if kind.is_dir() {
                if !relative.file_name().is_some_and(|name| name.to_string_lossy().starts_with('.')) {
                    folders.push(path);
                }
            } else if kind.is_file() && !left_out(relative) {
                let meta = entry.metadata().ok();
                let modified =
                    meta.as_ref().and_then(|meta| meta.modified().ok()).and_then(|time| time.duration_since(UNIX_EPOCH).ok());
                files.push(DiskFile {
                    path: relative.to_string_lossy().replace('\\', "/"),
                    bytes: meta.map_or(0, |meta| meta.len()),
                    modified: modified.map_or(0, |since| i64::try_from(since.as_secs()).unwrap_or(i64::MAX)),
                });
            }
        }
    }
    Ok(files)
}

fn lock(state: &Mutex<State>) -> std::sync::MutexGuard<'_, State> {
    state.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Watches `root`: a change the Gallery would show drops the kept list and calls `on_change`.
fn watch(root: &Path, state: Weak<Mutex<State>>, on_change: OnChange) -> Option<Debouncer<RecommendedWatcher>> {
    // macOS reports a linked folder's real path (`/var` is `/private/var`): events may come under either.
    let bases = [root.to_path_buf(), root.canonicalize().unwrap_or_else(|_| root.to_path_buf())];
    let relative = move |path: &Path| bases.iter().find_map(|base| path.strip_prefix(base).ok().map(Path::to_path_buf));
    let handler = move |result: DebounceEventResult| {
        let Ok(events) = result else { return };
        let shown = events.iter().any(|event| relative(&event.path).is_some_and(|relative| !left_out(&relative)));
        let Some(state) = state.upgrade().filter(|_| shown) else { return };
        lock(&state).files = None;
        on_change();
    };
    let mut debouncer = new_debouncer(QUIET, handler).map_err(|e| tracing::warn!("The library watcher didn't start: {e}")).ok()?;
    if let Err(error) = debouncer.watcher().watch(root, RecursiveMode::Recursive) {
        tracing::warn!("The library folder isn't watched: {error}");
        return None;
    }
    Some(debouncer)
}

// ── Public API ────────────────────────────
impl Disk {
    /// A listing that calls `on_change` whenever the watched library folder changes.
    pub fn new(on_change: OnChange) -> Self {
        Self { state: Arc::default(), on_change }
    }

    /// The files in the library at `root`, listed again when anything changed since; starts watching `root`, and
    /// moves the watch when the library moves.
    pub fn files(&self, root: &Path) -> Result<Arc<Vec<DiskFile>>, String> {
        let mut old = None;
        let found = {
            let mut state = lock(&self.state);
            if state.root.as_deref() != Some(root) {
                old = state.watcher.take();
                state.root = Some(root.to_path_buf());
                state.files = None;
                state.watcher = watch(root, Arc::downgrade(&self.state), self.on_change.clone());
            }
            match &state.files {
                Some(files) => Ok(files.clone()),
                None => list(root).map(|files| {
                    let files = Arc::new(files);
                    state.files = Some(files.clone());
                    files
                }),
            }
        };
        // Dropped outside the lock: it waits for its thread, which may be waiting for the lock.
        drop(old);
        found
    }
}

#[cfg(test)]
mod tests {
    use super::{Disk, left_out};
    use std::path::Path;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::{Duration, Instant};

    #[test]
    fn leaves_out_what_the_gallery_doesnt_show() {
        let out = |path: &str| left_out(Path::new(path));
        assert!(!out("Images/cats/a.png") && !out("Videos/x.mp4"));
        assert!(out(".scrappy/library.db") && out(".scrappy/thumbs/a.jpg") && out("Videos/x.mp4.part") && out("a.ytdl"));
        assert!(!out("Sites/a.com/index.html") && !out("Sites/a.com/_pages/blog-post.html"));
        assert!(out("Sites/a.com/css/site.css") && out("Sites/a.com/blog/index.html") && out("Sites/a.com/_pages/x.png"));
    }

    #[test]
    fn lists_the_folder_and_follows_changes() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join(".scrappy")).unwrap();
        std::fs::create_dir_all(root.join("Images/a")).unwrap();
        std::fs::write(root.join("Images/a/one.png"), "1").unwrap();
        std::fs::write(root.join(".scrappy/library.db"), "db").unwrap();
        let changes = Arc::new(AtomicUsize::new(0));
        let seen = changes.clone();
        let disk = Disk::new(Arc::new(move || {
            seen.fetch_add(1, Ordering::SeqCst);
        }));
        let paths = |disk: &Disk| disk.files(root).unwrap().iter().map(|file| file.path.clone()).collect::<Vec<_>>();
        assert_eq!(paths(&disk), ["Images/a/one.png"]);

        std::fs::write(root.join("Images/a/two.png"), "2").unwrap();
        std::fs::remove_file(root.join("Images/a/one.png")).unwrap();
        let started = Instant::now();
        while changes.load(Ordering::SeqCst) == 0 && started.elapsed() < Duration::from_secs(10) {
            std::thread::sleep(Duration::from_millis(50));
        }
        assert!(
            changes.load(Ordering::SeqCst) > 0,
            "the watcher saw the change"
        );
        assert_eq!(paths(&disk), ["Images/a/two.png"]);
    }
}

//! Relocate: moving the library to another folder as a job. Every file is copied, the copy is checked (each file
//! there, at its size), then the settings switch to it (ADR-0039). The old folder stays as it was; a stopped or failed
//! move removes only what it copied.

// ── Imports ───────────────────────────────
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::disk::OnChange;
use super::folder::{check, setup};
use crate::jobs::{Job, JobId, JobPhase, Jobs};
use crate::settings::Store;

// ── Constants ─────────────────────────────
/// Bytes copied between progress reports and cancel checks.
const BUFFER: usize = 8 * 1024 * 1024;

// ── Types ─────────────────────────────────
/// A file of the library: its path under the root, and its size.
struct Item {
    relative: PathBuf,
    size: u64,
}

// ── Private helpers ───────────────────────
/// Every file under `dir`, relative to `root`. Links aren't followed: they'd copy what's outside the library.
fn list(root: &Path, dir: &Path, items: &mut Vec<Item>) -> Result<(), String> {
    let failed = |e: std::io::Error| format!("Can't read the library: {e}");
    for entry in fs::read_dir(dir).map_err(failed)? {
        let entry = entry.map_err(failed)?;
        let kind = entry.file_type().map_err(failed)?;
        if kind.is_dir() {
            list(root, &entry.path(), items)?;
        } else if kind.is_file() {
            let size = entry.metadata().map_err(failed)?.len();
            let relative = entry.path().strip_prefix(root).map_err(|e| e.to_string())?.to_path_buf();
            items.push(Item { relative, size });
        }
    }
    Ok(())
}

/// Copies one file a buffer at a time, adding to `done` and reporting it, until `job` is stopped.
fn copy_file(from: &Path, to: &Path, done: &mut u64, total: u64, job: &Job) -> Result<(), String> {
    let failed = |e: std::io::Error| format!("Can't copy {}: {e}", from.display());
    if let Some(dir) = to.parent() {
        fs::create_dir_all(dir).map_err(failed)?;
    }
    let mut reader = File::open(from).map_err(failed)?;
    let mut writer = File::create(to).map_err(failed)?;
    let mut buffer = vec![0; BUFFER];
    loop {
        if job.cancelled() {
            return Err("Cancelled.".into());
        }
        let read = reader.read(&mut buffer).map_err(failed)?;
        if read == 0 {
            break;
        }
        writer.write_all(&buffer[..read]).map_err(failed)?;
        *done += read as u64;
        job.progress(JobPhase::Download, *done, Some(total));
    }
    writer.flush().map_err(failed)
}

/// Removes the files a move copied, then the folders it made that are left empty.
fn clean(target: &Path, copied: &[PathBuf]) {
    for file in copied {
        let _ = fs::remove_file(file);
    }
    let mut folders: Vec<&Path> = copied.iter().filter_map(|file| file.parent()).filter(|dir| *dir != target).collect();
    // Deepest first, so a folder is empty by the time it's tried; `remove_dir` leaves any that aren't.
    folders.sort_by_key(|dir| std::cmp::Reverse(dir.components().count()));
    folders.dedup();
    for dir in folders {
        for ancestor in dir.ancestors().take_while(|ancestor| *ancestor != target) {
            let _ = fs::remove_dir(ancestor);
        }
    }
}

/// Copies the library at `root` into `target` and checks the copy; on failure or a stop, removes what it copied.
fn copy_library(root: &Path, target: &Path, job: &Job) -> Result<(), String> {
    let mut items = Vec::new();
    list(root, root, &mut items)?;
    let total = items.iter().map(|item| item.size).sum();
    let mut copied = Vec::new();
    let mut done = 0;
    let result = items
        .iter()
        .try_for_each(|item| {
            let to = target.join(&item.relative);
            copied.push(to.clone());
            copy_file(&root.join(&item.relative), &to, &mut done, total, job)
        })
        .and_then(|()| {
            let size = |item: &Item| fs::metadata(target.join(&item.relative)).ok().map(|m| m.len());
            let wrong = items.iter().find(|item| size(item) != Some(item.size));
            wrong.map_or(Ok(()), |item| Err(format!("The copy of {} isn't complete.", item.relative.display())))
        });
    if result.is_err() {
        clean(target, &copied);
    }
    result
}

// ── Public API ────────────────────────────
/// Checks `input` as the place to move the library at `root` to: a new or empty folder outside the library, one
/// Scrappy can write to.
pub fn check_move(root: &Path, input: &str) -> Result<PathBuf, String> {
    let found = check(input)?;
    if !found.writable {
        return Err("Can't use this folder.".into());
    }
    if found.exists && !found.empty {
        return Err("Choose an empty folder.".into());
    }
    if found.path.starts_with(root) || root.starts_with(&found.path) {
        return Err("Choose a folder outside the library.".into());
    }
    Ok(found.path)
}

/// Starts moving the library to `input` as a job, unless downloads are `busy`; when the copy is checked, the settings
/// switch to it and `on_change` is called. Returns the job's id.
pub fn start_move(
    jobs: &Arc<Jobs>,
    settings: Store,
    input: &str,
    busy: bool,
    on_change: OnChange,
) -> Result<JobId, String> {
    if busy {
        return Err("Wait for the downloads to finish first.".into());
    }
    let root = settings.library_root()?;
    let target = check_move(&root, input)?;
    Ok(jobs.start(move |job| async move {
        let copy = {
            let (root, target) = (root.clone(), target.clone());
            tokio::task::spawn_blocking(move || copy_library(&root, &target, &job))
        };
        copy.await.map_err(|e| format!("The move stopped: {e}"))??;
        setup(&target.to_string_lossy(), &settings)?;
        on_change();
        tracing::info!("Library moved");
        Ok(())
    }))
}

#[cfg(test)]
mod tests {
    use super::{check_move, clean, start_move};
    use crate::jobs::{JobEvent, JobResult, Jobs};
    use crate::library::setup;
    use crate::settings::Store;
    use std::fs;
    use std::path::Path;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    fn text(path: &Path) -> &str {
        path.to_str().unwrap()
    }

    #[test]
    fn moves_only_into_an_empty_folder_outside_the_library() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("Library");
        fs::create_dir_all(&root).unwrap();
        let full = dir.path().join("Full");
        fs::create_dir_all(&full).unwrap();
        fs::write(full.join("a.txt"), "x").unwrap();
        assert_eq!(check_move(&root, text(&full)).unwrap_err(), "Choose an empty folder.");
        assert_eq!(check_move(&root, text(&root.join("Inside"))).unwrap_err(), "Choose a folder outside the library.");
        assert_eq!(check_move(&root, text(dir.path())).unwrap_err(), "Choose an empty folder.");
        assert_eq!(check_move(&root, text(&dir.path().join("New"))).unwrap(), dir.path().join("New"));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn copies_checks_and_switches_keeping_the_old_folder() {
        let dir = tempfile::tempdir().unwrap();
        let settings = Store::load(&dir.path().join("config"));
        let (old, new) = (dir.path().join("Old"), dir.path().join("Drive").join("New"));
        setup(text(&old), &settings).unwrap();
        fs::create_dir_all(old.join("Videos/someone")).unwrap();
        fs::write(old.join("Videos/someone/clip.mp4"), vec![7; 20_000_000]).unwrap();
        fs::write(old.join("Images/cat.png"), "png").unwrap();

        let finished = Arc::new(Mutex::new(None));
        let sink = finished.clone();
        let jobs = Jobs::new(Arc::new(move |event| {
            if let JobEvent::Finished(end) = event {
                *sink.lock().unwrap() = Some(end.result);
            }
        }));
        let changed = Arc::new(Mutex::new(false));
        let flag = changed.clone();
        start_move(&jobs, settings.clone(), text(&new), false, Arc::new(move || *flag.lock().unwrap() = true)).unwrap();
        for _ in 0..200 {
            if finished.lock().unwrap().is_some() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }

        assert_eq!(*finished.lock().unwrap(), Some(JobResult::Done));
        assert_eq!(fs::read(new.join("Videos/someone/clip.mp4")).unwrap().len(), 20_000_000);
        assert!(new.join("Images/cat.png").is_file() && new.join(".scrappy/library.db").is_file());
        assert!(old.join("Videos/someone/clip.mp4").is_file(), "the old folder stays");
        assert_eq!(settings.library_root().unwrap(), new);
        assert!(*changed.lock().unwrap());
        let busy = start_move(&jobs, settings, text(dir.path()), true, Arc::new(|| {}));
        assert_eq!(busy.unwrap_err(), "Wait for the downloads to finish first.");
    }

    #[test]
    fn a_stopped_move_removes_only_what_it_copied() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("New");
        fs::create_dir_all(target.join("Videos/a")).unwrap();
        fs::write(target.join("Videos/a/one.mp4"), "1").unwrap();
        fs::write(target.join("Videos/a/two.mp4"), "2").unwrap();
        fs::write(target.join("mine.txt"), "not copied").unwrap();
        clean(&target, &[target.join("Videos/a/one.mp4"), target.join("Videos/a/two.mp4")]);
        assert!(!target.join("Videos").exists(), "its emptied folders go");
        assert!(target.join("mine.txt").is_file() && target.is_dir());
    }
}

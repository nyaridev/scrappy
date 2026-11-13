//! Keep: a scan's finds saved as they're found (ADR-0031). Each find of the kept kinds goes into
//! `Grabs/<domain>/<date>/` through `media`, a few at a time, up to a number of files, skipping links the library has
//! and files under the smallest size; each find's state is kept for the window. Once the scan ends, the folder's extra
//! copies can go to the trash (ADR-0029).

// ── Imports ───────────────────────────────
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use ts_rs::TS;

use super::find::FindKind;
use super::save::{catalog_kind, folder};
use crate::dedupe;
use crate::jobs::Job;
use crate::library;
use crate::media::{Downloads, SaveRequest};
use crate::vault;

// ── Constants ─────────────────────────────
/// Files saved at once.
const AT_ONCE: usize = 4;
const STOP_CHECK: Duration = Duration::from_millis(200);

// ── Types ─────────────────────────────────
/// How saving a find went: one saved as it was found (ADR-0031), or a mirror's picked media (ADR-0033).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "state", rename_all = "camelCase")]
#[ts(export)]
pub enum Kept {
    Saving,
    Saved,
    /// Under the smallest size: not kept.
    Small,
    /// The library has it from before.
    Known,
    /// A video over the most a mirror saves.
    Big,
    Failed { message: String },
}

/// What to keep: the kinds, the smallest size and the most files.
pub(super) struct Wanted {
    pub kinds: Vec<FindKind>,
    pub min_bytes: u64,
    pub files: u32,
}

/// One scan's saving. Clones share it.
#[derive(Clone)]
pub(super) struct Keeper {
    downloads: Downloads,
    root: PathBuf,
    /// Relative to the library root.
    folder: PathBuf,
    site: Option<String>,
    wanted: Arc<Wanted>,
    known: Arc<HashSet<String>>,
    offered: Arc<Mutex<HashSet<String>>>,
    states: Arc<Mutex<HashMap<String, Kept>>>,
    gate: Arc<Semaphore>,
    saved: Arc<AtomicU32>,
    saves: Arc<Mutex<JoinSet<()>>>,
}

// ── Private helpers ───────────────────────
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

// ── Public API ────────────────────────────
impl Keeper {
    /// Saving for a scan of `page` into the library at `root`; streams aren't kept (they go through yt-dlp, picked).
    pub(super) fn new(downloads: Downloads, root: PathBuf, page: &str, mut wanted: Wanted) -> Result<Self, String> {
        wanted.kinds.retain(|kind| *kind != FindKind::Stream);
        let site = vault::site_of(page);
        Ok(Self {
            folder: folder(site.as_deref(), &library::today()?),
            known: Arc::new(library::sources(&root)?),
            downloads,
            root,
            site,
            wanted: Arc::new(wanted),
            offered: Arc::default(),
            states: Arc::default(),
            gate: Arc::new(Semaphore::new(AT_ONCE)),
            saved: Arc::default(),
            saves: Arc::default(),
        })
    }

    /// The folder it saves into, relative to the library root, with `/` separators.
    pub(super) fn folder(&self) -> String {
        self.folder.to_string_lossy().replace('\\', "/")
    }

    /// How many files it has saved.
    pub(super) fn saved(&self) -> u64 {
        u64::from(self.saved.load(Ordering::Relaxed))
    }

    /// It has saved as many files as it may.
    pub(super) fn full(&self) -> bool {
        self.saved.load(Ordering::Relaxed) >= self.wanted.files
    }

    /// Each find's state so far, by link.
    pub(super) fn states(&self) -> HashMap<String, Kept> {
        lock(&self.states).clone()
    }

    /// Saves the find at `url` (seen on `page`) when it's of a kept kind, new, and the file limit isn't reached.
    pub(super) fn offer(&self, url: &str, kind: FindKind, page: &str, job: &Job) {
        if !self.wanted.kinds.contains(&kind) || self.full() || !lock(&self.offered).insert(url.to_string()) {
            return;
        }
        if self.known.contains(url) {
            lock(&self.states).insert(url.to_string(), Kept::Known);
            return;
        }
        let request = SaveRequest {
            url: url.to_string(),
            page_url: page.to_string(),
            kind: catalog_kind(kind),
            stream: false,
            folder: self.folder.clone(),
            site: self.site.clone(),
            title: String::new(),
        };
        lock(&self.states).insert(url.to_string(), Kept::Saving);
        let (keeper, job, url) = (self.clone(), job.clone(), url.to_string());
        lock(&self.saves).spawn(async move {
            let Ok(_turn) = keeper.gate.acquire().await else { return };
            if job.cancelled() || keeper.full() {
                lock(&keeper.states).remove(&url);
                return;
            }
            let (root, min_bytes) = (keeper.root.clone(), keeper.wanted.min_bytes);
            let kept = match keeper.downloads.save_now(root, request, min_bytes, &job).await {
                Ok(Some(_)) => {
                    keeper.saved.fetch_add(1, Ordering::Relaxed);
                    Kept::Saved
                }
                Ok(None) => Kept::Small,
                Err(_) if job.cancelled() => {
                    lock(&keeper.states).remove(&url);
                    return;
                }
                Err(message) => Kept::Failed { message },
            };
            lock(&keeper.states).insert(url, kept);
        });
    }

    /// Waits for the saves started (stopped, it drops them), then moves the folder's extra copies to the trash when
    /// `remove_duplicates` is on. Returns how many files it saved.
    pub(super) async fn finish(&self, job: &Job, remove_duplicates: bool) -> Result<u64, String> {
        let mut saves = std::mem::take(&mut *lock(&self.saves));
        while !saves.is_empty() {
            tokio::select! {
                _ = saves.join_next() => {}
                () = tokio::time::sleep(STOP_CHECK) => if job.cancelled() { saves.abort_all(); },
            }
        }
        lock(&self.states).retain(|_, kept| *kept != Kept::Saving);
        if remove_duplicates && self.saved() > 1 && !job.cancelled() {
            let (root, folder, job) = (self.root.clone(), self.folder(), job.clone());
            tokio::task::spawn_blocking(move || dedupe::remove_copies(&root, &folder, &job))
                .await
                .map_err(|e| format!("The duplicate scan stopped: {e}"))??;
        }
        Ok(self.saved())
    }
}

//! Status: a running hunt as the window reads it: each source, what it's doing, the results it went through and the
//! images it saved (ADR-0024), then the duplicates removed at the end (ADR-0027). Only the latest hunt is kept.

// ── Imports ───────────────────────────────
use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use serde::Serialize;
use ts_rs::TS;

use crate::jobs::JobId;

// ── Types ─────────────────────────────────
/// What a source is doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum SourceState {
    /// Waiting for a free place, or for its turn to search.
    Waiting,
    Searching,
    Reading,
    Done,
    Failed,
    Stopped,
}

/// One source of a hunt.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct HuntSource {
    /// The image search engine, the site, or "Any site" for the web search.
    pub name: String,
    pub state: SourceState,
    /// Results (images to save and pages to read) gone through, of `found`.
    pub read: u32,
    pub found: u32,
    /// Images saved, of `limit`.
    pub saved: u32,
    pub limit: u32,
    /// Why it failed.
    pub problem: Option<String>,
}

/// A hunt: its sources, and its duplicates removed once they ended.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct HuntStatus {
    pub sources: Vec<HuntSource>,
    /// Removing duplicates, after every source ended.
    pub removing: bool,
    /// Duplicates moved to the trash; `None` until that's done, or when it's off.
    pub removed: Option<u32>,
}

/// One hunt's status, shared by its tasks.
pub(super) type Status = Arc<Mutex<HuntStatus>>;

/// The latest hunt's status, by its job. Clones share it.
#[derive(Clone, Default)]
pub struct Hunts {
    all: Arc<Mutex<HashMap<JobId, Status>>>,
}

// ── Private helpers ───────────────────────
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

// ── Public API ────────────────────────────
/// Changes source `index` of `status`.
pub(super) fn update(status: &Status, index: usize, change: impl FnOnce(&mut HuntSource)) {
    if let Some(source) = lock(status).sources.get_mut(index) {
        change(source);
    }
}

/// Changes the hunt's own part of `status`.
pub(super) fn change(status: &Status, change: impl FnOnce(&mut HuntStatus)) {
    change(&mut lock(status));
}

/// Images saved by every source, and how many sources are done.
pub(super) fn totals(status: &Status) -> (u64, usize) {
    let all = &lock(status).sources;
    let saved = all.iter().map(|source| u64::from(source.saved)).sum();
    (saved, all.iter().filter(|source| source.state == SourceState::Done).count())
}

impl Hunts {
    /// Keeps `status` as the hunt `id`'s, dropping older hunts.
    pub(super) fn begin(&self, id: JobId, status: Status) {
        let mut all = lock(&self.all);
        all.clear();
        all.insert(id, status);
    }

    /// The hunt `id` as it is now; `None` once a newer hunt started.
    pub fn status(&self, id: JobId) -> Option<HuntStatus> {
        lock(&self.all).get(&id).map(|status| lock(status).clone())
    }
}

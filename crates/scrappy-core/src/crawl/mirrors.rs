//! Mirrors: each mirror's record (ADR-0033): its pages and stylesheets, the files it saved, the images and videos its
//! pages show, and how saving each went. A crawl or a save works on it in memory; it's kept in
//! `<root>/.scrappy/mirrors/<site>.json`, so media can be picked after the crawl, even after a restart.

// ── Imports ───────────────────────────────
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::rewrite::Saved;
use crate::grab::{Find, Kept};
use crate::library::template::sanitize;

// ── Constants ─────────────────────────────
const RECORDS: &str = ".scrappy/mirrors";

// ── Types ─────────────────────────────────
/// One mirror. Addresses are kept without their `#fragment`; paths are relative to the mirror's folder.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(super) struct Mirror {
    /// The address the mirror started from.
    pub url: String,
    /// The mirror's folder, relative to the library root.
    pub folder: PathBuf,
    /// The site's host: other hosts' files go under `_hosts/`.
    pub host: String,
    pub pages: Vec<(String, PathBuf)>,
    pub sheets: Vec<(String, PathBuf)>,
    pub saved: Saved,
    /// The images and videos its pages show, in the order they were met.
    pub media: Vec<Find>,
    /// How saving each picked one went, by link.
    pub kept: HashMap<String, Kept>,
    /// A crawl or a save is running on it.
    #[serde(skip)]
    pub busy: bool,
}

/// A mirror's images and videos, for the window to pick from.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MirrorMedia {
    pub site: String,
    /// The address the mirror started from: the Referer of its media's thumbnails.
    pub url: String,
    /// Sorted by type, then name.
    pub finds: Vec<Find>,
    pub kept: HashMap<String, Kept>,
    /// A crawl or a save is running on it: no saving until it ends.
    pub busy: bool,
}

/// The mirrors read or worked on this session, by site. Clones share them.
#[derive(Clone, Default)]
pub struct Mirrors(Arc<Mutex<HashMap<String, Mirror>>>);

// ── Private helpers ───────────────────────
fn record_of(root: &Path, site: &str) -> PathBuf {
    root.join(RECORDS).join(format!("{}.json", sanitize(site)))
}

impl Mirrors {
    fn lock(&self) -> MutexGuard<'_, HashMap<String, Mirror>> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// `site`'s mirror: the one in memory, else its record in the library at `root`.
    pub(super) fn get(&self, root: &Path, site: &str) -> Result<Mirror, String> {
        if let Some(mirror) = self.lock().get(site) {
            return Ok(mirror.clone());
        }
        let text = std::fs::read(record_of(root, site)).map_err(|_| "There's no mirror of this site.".to_string())?;
        let unread = |e: serde_json::Error| format!("The mirror's record can't be read: {e}");
        let mirror: Mirror = serde_json::from_slice(&text).map_err(unread)?;
        self.lock().insert(site.to_string(), mirror.clone());
        Ok(mirror)
    }

    /// Starts `site`'s mirror over as `mirror`.
    pub(super) fn put(&self, site: &str, mirror: Mirror) {
        self.lock().insert(site.to_string(), mirror);
    }

    /// Changes `site`'s mirror in memory; one that isn't there is left alone.
    pub(super) fn update(&self, site: &str, change: impl FnOnce(&mut Mirror)) {
        if let Some(mirror) = self.lock().get_mut(site) {
            change(mirror);
        }
    }

    /// Writes `site`'s mirror to its record in the library at `root`.
    pub(super) fn store(&self, root: &Path, site: &str) -> Result<(), String> {
        let Some(json) = self.lock().get(site).map(serde_json::to_vec) else { return Ok(()) };
        let json = json.map_err(|e| e.to_string())?;
        let file = record_of(root, site);
        let failed = |e: std::io::Error| format!("Can't keep the mirror's record: {e}");
        std::fs::create_dir_all(file.parent().unwrap_or(root)).map_err(failed)?;
        let part = file.with_extension("json.part");
        std::fs::write(&part, json).map_err(failed)?;
        std::fs::rename(&part, &file).map_err(failed)
    }
}

// ── Public API ────────────────────────────
impl Mirrors {
    /// `site`'s images and videos, and how saving each went, from the library at `root`.
    pub fn media(&self, root: &Path, site: &str) -> Result<MirrorMedia, String> {
        let mirror = self.get(root, site)?;
        let mut finds = mirror.media;
        finds.sort_by(|a, b| (a.kind, &a.name, &a.url).cmp(&(b.kind, &b.name, &b.url)));
        Ok(MirrorMedia { site: site.to_string(), url: mirror.url, finds, kept: mirror.kept, busy: mirror.busy })
    }
}

#[cfg(test)]
mod tests {
    use super::{Mirror, Mirrors};
    use crate::grab::{Find, FindKind, Kept};

    #[test]
    fn keeps_a_mirror_across_restarts() {
        let dir = tempfile::tempdir().unwrap();
        let find = |url: &str, name: &str, kind| Find { url: url.into(), kind, name: name.into(), size: None };
        let media =
            vec![find("https://a.com/b.jpg", "b.jpg", FindKind::Image), find("https://a.com/v.mp4", "v.mp4", FindKind::Video)];
        let mirror = Mirror { url: "https://a.com/".into(), media, busy: true, ..Mirror::default() };
        let mirrors = Mirrors::default();
        mirrors.put("a.com", mirror);
        mirrors.update("a.com", |mirror| {
            mirror.kept.insert("https://a.com/v.mp4".into(), Kept::Big);
        });
        mirrors.store(dir.path(), "a.com").unwrap();

        let media = Mirrors::default().media(dir.path(), "a.com").unwrap();
        assert_eq!(media.finds.iter().map(|find| find.name.as_str()).collect::<Vec<_>>(), ["v.mp4", "b.jpg"]);
        assert_eq!(media.kept["https://a.com/v.mp4"], Kept::Big);
        assert!(!media.busy, "a mirror read back isn't running");
        assert!(Mirrors::default().media(dir.path(), "b.com").unwrap_err().contains("no mirror"));
    }
}

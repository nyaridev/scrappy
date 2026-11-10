//! Save: the finds the user picked, queued as downloads into `Grabs/<domain>/<date>/` (ADR-0004, ADR-0007). Files are
//! fetched as they are; streams go through yt-dlp.

// ── Imports ───────────────────────────────
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::find::{FindKind, MAX_FINDS};
use crate::jobs::{JobId, Jobs};
use crate::library::{self, template};
use crate::media::{Downloads, SaveRequest};
use crate::vault;

// ── Constants ─────────────────────────────
const FOLDER: &str = "Grabs/{site}/{upload_date}";

// ── Types ─────────────────────────────────
/// What the window asks Grab to download.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct GrabRequest {
    /// The page the finds are from.
    pub page_url: String,
    /// The page's title: a stream's file name.
    pub title: Option<String>,
    pub finds: Vec<Pick>,
}

/// One picked find.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Pick {
    pub url: String,
    pub kind: FindKind,
    pub name: String,
}

/// How one pick went: its download's id, or why it couldn't start.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Picked {
    pub url: String,
    pub id: Option<JobId>,
    pub error: Option<String>,
}

// ── Private helpers ───────────────────────
fn catalog_kind(kind: FindKind) -> &'static str {
    match kind {
        FindKind::Video | FindKind::Stream => "video",
        FindKind::Audio => "audio",
        FindKind::Image => "image",
        FindKind::Other => "file",
    }
}

/// `Grabs/<domain>/<date>`; the date folder alone when the page has no domain.
fn folder(site: Option<&str>, today: &str) -> PathBuf {
    let fields = template::Fields {
        site: site.map(str::to_string),
        upload_date: Some(today.to_string()),
        ..template::Fields::default()
    };
    template::render(FOLDER, &fields)
}

/// A stream's name: the page's title, else the find's name without its extension.
fn stream_title(title: Option<&str>, name: &str) -> String {
    let stem = Path::new(name).file_stem().map_or_else(|| name.to_string(), |s| s.to_string_lossy().into_owned());
    title.map(str::trim).filter(|title| !title.is_empty()).map_or(stem, str::to_string)
}

// ── Public API ────────────────────────────
/// Queues each pick in `request` into the library at `root`. One that can't start doesn't stop the rest.
pub async fn save(
    downloads: &Downloads,
    jobs: &Arc<Jobs>,
    root: PathBuf,
    request: GrabRequest,
) -> Result<Vec<Picked>, String> {
    if request.finds.len() > MAX_FINDS {
        return Err(format!("Pick at most {MAX_FINDS} at a time."));
    }
    let site = vault::site_of(&request.page_url);
    let folder = folder(site.as_deref(), &library::today()?);
    let mut picked = Vec::with_capacity(request.finds.len());
    for pick in request.finds {
        let stream = pick.kind == FindKind::Stream;
        let save = SaveRequest {
            url: pick.url.clone(),
            page_url: request.page_url.clone(),
            kind: catalog_kind(pick.kind),
            stream,
            folder: folder.clone(),
            site: site.clone(),
            title: if stream { stream_title(request.title.as_deref(), &pick.name) } else { pick.name },
        };
        let (id, error) = match downloads.save(jobs, root.clone(), save).await {
            Ok(entry) => (Some(entry.id), None),
            Err(error) => (None, Some(error)),
        };
        picked.push(Picked { url: pick.url, id, error });
    }
    Ok(picked)
}

#[cfg(test)]
mod tests {
    use super::{GrabRequest, Pick, folder, save, stream_title};
    use crate::grab::FindKind;
    use crate::jobs::{JobEvent, Jobs};
    use crate::media::{DownloadState, Downloads};
    use crate::tools::Toolbox;
    use crate::vault::{KeySource, Vault};
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    /// Saves a picked video into `Grabs/`: run with `cargo test -p scrappy-core -- --ignored`.
    #[tokio::test]
    #[ignore = "downloads a small video"]
    async fn saves_a_picked_file() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("Library");
        std::fs::create_dir_all(root.join(".scrappy")).unwrap();
        let finished = Arc::new(Mutex::new(0));
        let sink = finished.clone();
        let jobs = Jobs::new(Arc::new(move |event| {
            if let JobEvent::Finished(_) = event {
                *sink.lock().unwrap() += 1;
            }
        }));
        let vault = Vault::open(&dir.path().join("vault"), KeySource::Fixed([1; 32]));
        let downloads = Downloads::new(Toolbox::new(dir.path().join("tools")), vault);
        let pick = Pick { url: "https://www.w3schools.com/html/mov_bbb.mp4".into(), kind: FindKind::Video, name: "mov_bbb.mp4".into() };
        let request =
            GrabRequest { page_url: "https://www.w3schools.com/html/html5_video.asp".into(), title: None, finds: vec![pick] };
        let picked = save(&downloads, &jobs, root.clone(), request).await.unwrap();
        assert!(picked[0].id.is_some(), "{picked:?}");
        while *finished.lock().unwrap() < 1 {
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
        let entry = downloads.list().remove(0);
        assert_eq!(entry.state, DownloadState::Done, "{entry:?}");
        let file = entry.file.unwrap();
        assert!(file.starts_with("Grabs/w3schools.com/") && file.ends_with("/mov_bbb.mp4"), "{file}");
        assert!(root.join(file).is_file());
    }

    #[test]
    fn files_go_under_the_domain_and_date() {
        assert_eq!(folder(Some("example.com"), "2026-11-10"), PathBuf::from("Grabs/example.com/2026-11-10"));
        assert_eq!(folder(None, "2026-11-10"), PathBuf::from("Grabs/2026-11-10"));
    }

    #[test]
    fn names_a_stream_after_its_page() {
        assert_eq!(stream_title(Some(" Launch day "), "master.m3u8"), "Launch day");
        assert_eq!(stream_title(None, "master.m3u8"), "master");
        assert_eq!(stream_title(Some(""), "index.mpd"), "index");
    }
}

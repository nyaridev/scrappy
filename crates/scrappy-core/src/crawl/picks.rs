//! Picks: a mirror's picked images and videos saved into its folder (ADR-0033), a few at a time, each streamed to disk
//! at its own local path. A video over the most it keeps is skipped, and a file the mirror has is kept as it is. Then
//! the mirror's pages and stylesheets point at the new copies.

// ── Imports ───────────────────────────────
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use reqwest::header::REFERER;
use reqwest::{Client, Response, Url};
use serde::Deserialize;
use tokio::io::AsyncWriteExt;
use tokio::task::JoinSet;
use ts_rs::TS;

use super::local::local_path;
use super::mirror::rewrite_all;
use super::mirrors::Mirrors;
use crate::grab::{FindKind, Kept};
use crate::jobs::{Job, JobId, Jobs};
use crate::media::{DownloadSource, Downloads, Row};
use crate::net;

// ── Constants ─────────────────────────────
/// Files saved at once.
const AT_ONCE: usize = 4;
/// A file that sends nothing for this long has failed; a long video may take as long as it needs.
const READ_TIMEOUT: Duration = Duration::from_secs(60);
const MAX_VIDEO_MB: u32 = 100_000;

// ── Types ─────────────────────────────────
/// The media to save from a mirror.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MediaRequest {
    pub site: String,
    /// Links from the mirror's media list; any other is ignored.
    pub urls: Vec<String>,
    /// Videos over this many megabytes are skipped.
    pub max_video_mb: u32,
}

// ── Private helpers ───────────────────────
fn failed(what: &str, error: impl std::fmt::Display) -> Kept {
    Kept::Failed { message: format!("{what}: {error}") }
}

/// Writes `response` into `part` as it comes, up to `limit` bytes.
async fn stream(response: &mut Response, part: &Path, limit: Option<u64>, job: &Job) -> Result<(), Kept> {
    let mut out = tokio::fs::File::create(part).await.map_err(|e| failed("Can't save it", e))?;
    let mut bytes = 0u64;
    while let Some(chunk) = response.chunk().await.map_err(|e| failed("It didn't download", e.without_url()))? {
        if job.cancelled() {
            return Err(Kept::Failed { message: "Stopped.".into() });
        }
        bytes += chunk.len() as u64;
        if limit.is_some_and(|limit| bytes > limit) {
            return Err(Kept::Big);
        }
        out.write_all(&chunk).await.map_err(|e| failed("Can't save it", e))?;
    }
    out.flush().await.map_err(|e| failed("Can't save it", e))
}

/// Saves `url` as `file`, sent from `page`; a video over `limit` bytes is skipped. A file that's there is kept.
async fn fetch(
    client: Client,
    job: Job,
    file: PathBuf,
    url: String,
    page: String,
    limit: Option<u64>,
) -> Result<(), Kept> {
    if file.is_file() {
        return Ok(());
    }
    let request = client.get(&url).header(REFERER, page);
    let mut response = request
        .send()
        .await
        .and_then(|response| response.error_for_status())
        .map_err(|e| failed("It didn't download", e.without_url()))?;
    if limit.zip(response.content_length()).is_some_and(|(limit, length)| length > limit) {
        return Err(Kept::Big);
    }
    if let Some(parent) = file.parent() {
        tokio::fs::create_dir_all(parent).await.map_err(|e| failed("Can't make its folder", e))?;
    }
    let part = PathBuf::from(format!("{}.part", file.display()));
    match stream(&mut response, &part, limit, &job).await {
        Ok(()) => tokio::fs::rename(&part, &file).await.map_err(|e| failed("Can't save it", e)),
        Err(kept) => {
            let _ = tokio::fs::remove_file(&part).await;
            Err(kept)
        }
    }
}

/// Saves `picks` (links with their kinds) of `site`'s mirror, reporting each one's state into `mirrors`; then points
/// the mirror's pages at them. Returns how many were saved.
async fn save(
    root: &Path,
    site: &str,
    picks: Vec<(String, FindKind)>,
    limit: u64,
    job: &Job,
    mirrors: &Mirrors,
) -> Result<u64, String> {
    let mirror = mirrors.get(root, site)?;
    let dir = root.join(&mirror.folder);
    let client = net::client()
        .read_timeout(READ_TIMEOUT)
        .build()
        .map_err(|e| format!("Can't start saving: {e}"))?;
    let total = picks.len() as u64;
    let mut waiting = picks.into_iter();
    let mut fetches = JoinSet::new();
    let (mut done, mut saved) = (0u64, 0u64);
    loop {
        while !job.cancelled()
            && fetches.len() < AT_ONCE
            && let Some((url, kind)) = waiting.next()
        {
            let Ok(address) = Url::parse(&url) else { continue };
            let local = local_path(&address, &mirror.host, false);
            let file = dir.join(&local);
            let video = (kind == FindKind::Video).then_some(limit);
            let (client, job, page) = (client.clone(), job.clone(), mirror.url.clone());
            fetches.spawn(async move { (url.clone(), local, fetch(client, job, file, url, page, video).await) });
        }
        let Some(next) = fetches.join_next().await else { break };
        let Ok((url, local, result)) = next else { continue };
        done += 1;
        saved += u64::from(result.is_ok());
        let stopped = job.cancelled();
        mirrors.update(site, |mirror| match result {
            Ok(()) => {
                mirror.saved.insert(url.clone(), local);
                mirror.kept.insert(url, Kept::Saved);
            }
            // One cut short by Stop can be picked again.
            Err(_) if stopped => {
                mirror.kept.remove(&url);
            }
            Err(kept) => {
                mirror.kept.insert(url, kept);
            }
        });
        job.items(done, Some(total), None);
    }
    let left: Vec<String> = waiting.map(|(url, _)| url).collect();
    mirrors.update(site, |mirror| left.iter().for_each(|url| drop(mirror.kept.remove(url))));
    job.items(done, Some(total), Some("Updating links".into()));
    let mirror = mirrors.get(root, site)?;
    tokio::task::spawn_blocking(move || rewrite_all(&dir, &mirror.pages, &mirror.sheets, &mirror.saved, true))
        .await
        .map_err(|_| "The link update stopped.".to_string())?;
    if job.cancelled() {
        return Err("Cancelled.".into());
    }
    tracing::info!("A mirror saved {saved} of {total} picked files");
    Ok(saved)
}

// ── Public API ────────────────────────────
/// Starts saving `request`'s picks from its mirror in the library at `root`, as a job with a row in the Download list.
/// Only links the mirror listed are fetched. Progress reports the picks done of all of them; `mirrors` has each one's
/// state as it goes. Must be called inside a Tokio runtime.
pub fn save_media(
    mirrors: &Mirrors,
    downloads: &Downloads,
    jobs: &Arc<Jobs>,
    root: PathBuf,
    request: MediaRequest,
) -> Result<JobId, String> {
    let site = request.site;
    let mirror = mirrors.get(&root, &site)?;
    if mirror.busy {
        return Err("Wait for the mirror to finish.".into());
    }
    let listed: HashMap<&str, FindKind> = mirror.media.iter().map(|find| (find.url.as_str(), find.kind)).collect();
    let mut picks: Vec<(String, FindKind)> = Vec::new();
    for url in request.urls {
        if let Some(kind) = listed.get(url.as_str())
            && !picks.iter().any(|(picked, _)| *picked == url)
        {
            picks.push((url, *kind));
        }
    }
    if picks.is_empty() {
        return Err("Pick something to save.".into());
    }
    let limit = u64::from(request.max_video_mb.clamp(1, MAX_VIDEO_MB)) * 1024 * 1024;
    mirrors.update(&site, |mirror| {
        mirror.busy = true;
        picks.iter().for_each(|(url, _)| drop(mirror.kept.insert(url.clone(), Kept::Saving)));
    });
    let folder = mirror.folder.to_string_lossy().replace('\\', "/");
    let (url, title) = (mirror.url, site.clone());
    let row = Row { source: DownloadSource::Scrape, url, title, kind: "Media".into(), thumbnail: None };
    let mirrors = mirrors.clone();
    Ok(downloads.track(jobs, row, folder, move |job| async move {
        let result = save(&root, &site, picks, limit, &job, &mirrors).await;
        mirrors.update(&site, |mirror| mirror.busy = false);
        if let Err(error) = mirrors.store(&root, &site) {
            tracing::warn!("{error}");
        }
        result
    }))
}

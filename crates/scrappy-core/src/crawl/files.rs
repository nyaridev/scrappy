//! Files: a crawl for a site's files. `spider` walks the site within its limits (same site, depth, pages, a delay,
//! `robots.txt`); each page's file links of the wanted kinds are saved through `media` into `Images/<domain>/`, a few at
//! a time, skipping links the catalog already has, until the file limit (ADR-0008).

// ── Imports ───────────────────────────────
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use reqwest::Url;
use tokio::sync::Semaphore;
use tokio::sync::broadcast::error::RecvError;
use tokio::task::JoinSet;

use super::links::file_links;
use super::request::{Plan, pages, website};
use crate::grab::{FindKind, classify};
use crate::jobs::Job;
use crate::library;
use crate::media::{Downloads, SaveRequest};
use crate::vault;

// ── Constants ─────────────────────────────
/// Files saved at once.
const AT_ONCE: usize = 4;
const CANCEL_CHECK: Duration = Duration::from_millis(200);

// ── Private helpers ───────────────────────
fn catalog_kind(kind: FindKind) -> &'static str {
    match kind {
        FindKind::Video | FindKind::Stream => "video",
        FindKind::Audio => "audio",
        FindKind::Image => "image",
        FindKind::Other => "file",
    }
}

// ── Public API ────────────────────────────
/// Crawls and saves until the site, the page limit or the file limit runs out, or the job is stopped.
pub(super) async fn run(downloads: Downloads, root: PathBuf, plan: Plan, job: Job) -> Result<(), String> {
    let known = library::sources(&root)?;
    let mut website = website(&plan);
    let mut found = website.subscribe(64);
    let crawl = tokio::spawn(async move { website.crawl().await });

    let plan = Arc::new(plan);
    let gate = Arc::new(Semaphore::new(AT_ONCE));
    let saved = Arc::new(AtomicU32::new(0));
    let mut seen = HashSet::new();
    let mut visited = 0u32;
    let mut saves = JoinSet::new();
    let report = |visited: u32, saved: u32| {
        job.items(u64::from(saved), Some(u64::from(plan.files)), Some(pages(visited as usize)));
    };
    loop {
        let full = saved.load(Ordering::Relaxed) >= plan.files;
        if job.cancelled() || full {
            crawl.abort();
            break;
        }
        tokio::select! {
            page = found.recv() => match page {
                Ok(page) => {
                    visited += 1;
                    let Ok(address) = Url::parse(page.get_url()) else { continue };
                    for link in file_links(&address, &page.get_html()) {
                        let Some(kind) = classify(&link, None).filter(|kind| plan.kinds.contains(kind)) else { continue };
                        if known.contains(&link) || !seen.insert(link.clone()) {
                            continue;
                        }
                        let request = SaveRequest {
                            url: link,
                            page_url: address.to_string(),
                            kind: catalog_kind(kind),
                            stream: false,
                            folder: plan.folder.clone(),
                            site: Some(plan.site.clone()),
                            title: String::new(),
                        };
                        let (downloads, root, job) = (downloads.clone(), root.clone(), job.clone());
                        let (gate, saved, plan) = (gate.clone(), saved.clone(), plan.clone());
                        saves.spawn(async move {
                            let Ok(_turn) = gate.acquire().await else { return };
                            if job.cancelled() || saved.load(Ordering::Relaxed) >= plan.files {
                                return;
                            }
                            let url = request.url.clone();
                            match downloads.save_now(root, request, plan.min_bytes, &job).await {
                                Ok(Some(_)) => {
                                    saved.fetch_add(1, Ordering::Relaxed);
                                }
                                Ok(None) => {}
                                Err(error) => tracing::warn!("A crawled file didn't save ({}): {error}", vault::site_of(&url).unwrap_or_default()),
                            }
                        });
                    }
                    report(visited, saved.load(Ordering::Relaxed));
                }
                Err(RecvError::Lagged(missed)) => tracing::warn!("The crawl skipped {missed} pages"),
                Err(RecvError::Closed) => break,
            },
            () = tokio::time::sleep(CANCEL_CHECK) => report(visited, saved.load(Ordering::Relaxed)),
        }
    }
    // The pages are all in; the saves started finish, unless the job is stopped.
    while !saves.is_empty() {
        tokio::select! {
            _ = saves.join_next() => report(visited, saved.load(Ordering::Relaxed)),
            () = tokio::time::sleep(CANCEL_CHECK) => if job.cancelled() { saves.abort_all(); },
        }
    }
    if job.cancelled() {
        return Err("Cancelled.".into());
    }
    if visited == 0 {
        return Err("The site gave no pages. It may be down, or its robots.txt may not allow crawling.".into());
    }
    tracing::info!("A crawl visited {visited} pages and saved {} files", saved.load(Ordering::Relaxed));
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::crawl::test_site::{request, serve};
    use crate::crawl::{CrawlMode, crawl};
    use crate::jobs::{JobEvent, JobResult, Jobs};
    use crate::media::Downloads;
    use crate::tools::Toolbox;
    use crate::vault::{KeySource, Vault};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    /// Two pages, a big and a tiny picture, and a page `robots.txt` keeps the crawler out of.
    fn route(path: &str, _: &str) -> Option<(&'static str, Vec<u8>)> {
        Some(match path {
            "/" => ("text/html", br#"<a href="/two">two</a><img src="/big.png"><img src="/tiny.png"><a href="/private/">p</a>"#.to_vec()),
            "/two" => ("text/html", br#"<img src="/also.png"><img src="/big.png">"#.to_vec()),
            "/private/" => ("text/html", br#"<img src="/secret.png">"#.to_vec()),
            "/robots.txt" => ("text/plain", b"User-agent: *\nDisallow: /private/\n".to_vec()),
            "/big.png" | "/also.png" | "/secret.png" => ("image/png", vec![7u8; 4096]),
            "/tiny.png" => ("image/png", vec![7u8; 100]),
            _ => return None,
        })
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn crawls_within_its_limits() {
        let address = serve(route).await;
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("Library");
        std::fs::create_dir_all(root.join(".scrappy")).unwrap();
        let ended = Arc::new(Mutex::new(Vec::new()));
        let sink = ended.clone();
        let jobs = Jobs::new(Arc::new(move |event| {
            if let JobEvent::Finished(end) = event {
                sink.lock().unwrap().push(end.result);
            }
        }));
        let vault = Vault::open(&dir.path().join("vault"), KeySource::Fixed([1; 32]));
        let downloads = Downloads::new(Toolbox::new(dir.path().join("tools")), vault);

        let started = crawl(&downloads, &jobs, root.clone(), request(&address, CrawlMode::Files)).unwrap();
        while ended.lock().unwrap().is_empty() {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        assert_eq!(ended.lock().unwrap()[0], JobResult::Done);
        let folder = root.join(&started.folder);
        let mut names: Vec<String> =
            std::fs::read_dir(&folder).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
        names.sort();
        // The tiny one is under 1 KB, and robots.txt keeps the private page out.
        assert_eq!(names, ["also.png", "big.png"]);

        // A second crawl skips what the catalog has.
        ended.lock().unwrap().clear();
        crawl(&downloads, &jobs, root.clone(), request(&address, CrawlMode::Files)).unwrap();
        while ended.lock().unwrap().is_empty() {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        assert_eq!(std::fs::read_dir(&folder).unwrap().count(), 2);
    }
}

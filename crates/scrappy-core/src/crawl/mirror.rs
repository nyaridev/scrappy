//! Mirror: a site saved for offline use. The crawler walks it within its limits; each page goes into `Sites/<domain>/`
//! with the stylesheets, scripts, images and fonts it uses (and those its stylesheets use), one local path per address;
//! then every saved page and stylesheet is rewritten to point at the copies (ADR-0008).

// ── Imports ───────────────────────────────
use std::collections::{HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::time::Duration;

use reqwest::header::CONTENT_TYPE;
use reqwest::{Client, Url};
use tokio::sync::broadcast::error::RecvError;
use tokio::task::JoinSet;

use super::links::{asset_links, css_links};
use super::local::local_path;
use super::request::{Plan, pages, website};
use super::rewrite::{Saved, rewrite_css, rewrite_html};
use crate::app::USER_AGENT;
use crate::jobs::Job;
use crate::library::{self, Record};

// ── Constants ─────────────────────────────
/// Files fetched at once.
const AT_ONCE: usize = 4;
/// shortcut: a file is held in memory before it's written, so big ones are skipped; stream them if mirrors need video.
const MAX_FILE_BYTES: u64 = 50 * 1024 * 1024;
const FILE_TIMEOUT: Duration = Duration::from_secs(60);
const CANCEL_CHECK: Duration = Duration::from_millis(200);

// ── Types ─────────────────────────────────
/// A file the mirror saved.
struct Fetched {
    url: Url,
    local: PathBuf,
    bytes: u64,
    /// A stylesheet's own links; `None` for any other file.
    css: Option<Vec<String>>,
}

// ── Private helpers ───────────────────────
/// `url` without its `#fragment`: one address, one copy.
fn key(url: &str) -> Option<String> {
    let mut url = Url::parse(url).ok()?;
    url.set_fragment(None);
    Some(url.into())
}

async fn write(dir: &Path, local: &Path, bytes: &[u8]) -> Result<(), String> {
    let file = dir.join(local);
    if let Some(parent) = file.parent() {
        tokio::fs::create_dir_all(parent).await.map_err(|e| format!("Can't make a mirror folder: {e}"))?;
    }
    tokio::fs::write(&file, bytes).await.map_err(|e| format!("Can't save a mirror file: {e}"))
}

/// Fetches `url` into the mirror at `dir`.
async fn fetch(client: Client, dir: PathBuf, host: String, url: Url) -> Result<Fetched, String> {
    let failed = |e: reqwest::Error| format!("A mirror file didn't download: {}", e.without_url());
    let response = client.get(url.clone()).send().await.and_then(|r| r.error_for_status()).map_err(failed)?;
    let too_big = || "A mirror file is too big to keep.".to_string();
    if response.content_length().is_some_and(|length| length > MAX_FILE_BYTES) {
        return Err(too_big());
    }
    let css = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.starts_with("text/css"))
        || url.path().to_ascii_lowercase().ends_with(".css");
    let body = response.bytes().await.map_err(failed)?;
    if body.len() as u64 > MAX_FILE_BYTES {
        return Err(too_big());
    }
    let local = local_path(&url, &host, false);
    write(&dir, &local, &body).await?;
    let css = css.then(|| css_links(&url, &String::from_utf8_lossy(&body)));
    Ok(Fetched { url, local, bytes: body.len() as u64, css })
}

/// Queues the links not seen yet, while the file limit allows.
fn queue(links: Vec<String>, limit: u32, seen: &mut HashSet<String>, waiting: &mut VecDeque<Url>) {
    for link in links.iter().filter_map(|link| key(link)) {
        if seen.len() >= limit as usize {
            return;
        }
        if seen.insert(link.clone())
            && let Ok(url) = Url::parse(&link)
        {
            waiting.push_back(url);
        }
    }
}

/// Points every saved page and stylesheet at the copies. A stylesheet that isn't UTF-8 stays as it was.
fn rewrite_all(dir: &Path, pages: &[(Url, PathBuf)], sheets: &[(Url, PathBuf)], saved: &Saved) {
    let files = pages.iter().map(|(url, local)| (url, local, true)).chain(sheets.iter().map(|(url, local)| (url, local, false)));
    for (url, local, page) in files {
        let file = dir.join(local);
        let Ok(text) = std::fs::read_to_string(&file) else { continue };
        let text = if page { rewrite_html(&text, url, local, saved) } else { rewrite_css(&text, url, local, saved) };
        if let Err(error) = std::fs::write(&file, text) {
            tracing::warn!("A mirror file wasn't rewritten: {error}");
        }
    }
}

// ── Public API ────────────────────────────
/// Where a mirror of `url` keeps its start page, relative to the mirror's folder.
pub(super) fn start_page(url: &Url) -> PathBuf {
    local_path(url, url.host_str().unwrap_or_default(), true)
}

/// Mirrors the site until it, the page limit or the job runs out. Progress reports files saved of the file limit,
/// with the pages saved as its message. Ends with the mirror's start page in the catalog as one `site` row.
pub(super) async fn run(root: PathBuf, plan: Plan, job: Job) -> Result<(), String> {
    let dir = root.join(&plan.folder);
    let host = plan.url.host_str().unwrap_or_default().to_string();
    let client = Client::builder()
        .user_agent(USER_AGENT)
        .timeout(FILE_TIMEOUT)
        .build()
        .map_err(|e| format!("Can't start the mirror: {e}"))?;
    let mut website = website(&plan);
    let mut found = website.subscribe(64);
    let crawl = tokio::spawn(async move { website.crawl().await });

    let mut saved = Saved::new();
    let mut page_files: Vec<(Url, PathBuf)> = Vec::new();
    let mut sheets: Vec<(Url, PathBuf)> = Vec::new();
    let mut seen = HashSet::new();
    let mut waiting = VecDeque::new();
    let mut fetches = JoinSet::new();
    let (mut files, mut bytes, mut crawling) = (0u64, 0u64, true);
    let report = |page_count: usize, files: u64| job.items(files, Some(u64::from(plan.files)), Some(pages(page_count)));
    loop {
        if job.cancelled() {
            crawl.abort();
            fetches.abort_all();
            return Err("Cancelled.".into());
        }
        while fetches.len() < AT_ONCE
            && let Some(url) = waiting.pop_front()
        {
            fetches.spawn(fetch(client.clone(), dir.clone(), host.clone(), url));
        }
        if !crawling && fetches.is_empty() {
            break;
        }
        tokio::select! {
            page = found.recv(), if crawling => match page {
                Ok(page) => {
                    let Ok(address) = Url::parse(page.get_url()) else { continue };
                    let html = page.get_html();
                    // The first page is the start page, wherever a redirect took it.
                    let first = page_files.is_empty();
                    let local = if first { start_page(&plan.url) } else { local_path(&address, &host, true) };
                    if let Err(error) = write(&dir, &local, html.as_bytes()).await {
                        tracing::warn!("{error}");
                        continue;
                    }
                    bytes += html.len() as u64;
                    if first && let Some(start) = key(plan.url.as_str()) {
                        saved.insert(start, local.clone());
                    }
                    if let Some(own) = key(address.as_str()) {
                        saved.insert(own, local.clone());
                    }
                    queue(asset_links(&address, &html), plan.files, &mut seen, &mut waiting);
                    page_files.push((address, local));
                    report(page_files.len(), files);
                }
                Err(RecvError::Lagged(missed)) => tracing::warn!("The mirror skipped {missed} pages"),
                Err(RecvError::Closed) => crawling = false,
            },
            Some(done) = fetches.join_next(), if !fetches.is_empty() => {
                match done {
                    Ok(Ok(file)) => {
                        files += 1;
                        bytes += file.bytes;
                        if let Some(links) = file.css {
                            queue(links, plan.files, &mut seen, &mut waiting);
                            sheets.push((file.url.clone(), file.local.clone()));
                        }
                        saved.insert(file.url.into(), file.local);
                    }
                    Ok(Err(error)) => tracing::warn!("{error}"),
                    Err(_) => {}
                }
                report(page_files.len(), files);
            },
            () = tokio::time::sleep(CANCEL_CHECK) => report(page_files.len(), files),
        }
    }
    let Some((_, start)) = page_files.first().cloned() else {
        return Err("The site gave no pages. It may be down, or its robots.txt may not allow crawling.".into());
    };
    job.items(files, Some(u64::from(plan.files)), Some("Updating links".into()));
    let rewrite_dir = dir.clone();
    tokio::task::spawn_blocking(move || rewrite_all(&rewrite_dir, &page_files, &sheets, &saved))
        .await
        .map_err(|_| "The link update stopped.".to_string())?;
    library::record(
        &root,
        &Record {
            path: plan.folder.join(start).to_string_lossy().replace('\\', "/"),
            kind: "site",
            source_url: plan.url.to_string(),
            site: Some(plan.site.clone()),
            user: None,
            size_bytes: bytes,
            duration_secs: None,
            codec: None,
            transferred_bytes: bytes,
        },
    )?;
    tracing::info!("A mirror saved {files} files");
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::crawl::test_site::{request, serve};
    use crate::crawl::{CrawlMode, crawl};
    use crate::jobs::{JobEvent, JobResult, Jobs};
    use crate::library::{self, GalleryQuery, GallerySort};
    use crate::media::Downloads;
    use crate::tools::Toolbox;
    use crate::vault::{KeySource, Vault};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    /// Two pages sharing a stylesheet that imports another and uses a picture.
    fn route(path: &str, _: &str) -> Option<(&'static str, Vec<u8>)> {
        Some(match path {
            "/" => (
                "text/html",
                br#"<link rel="stylesheet" href="/css/site.css"><a href="/blog/post">post</a><img src="/img/a.png">"#.to_vec(),
            ),
            "/blog/post" => ("text/html", br#"<link rel="stylesheet" href="/css/site.css"><a href="/">home</a>"#.to_vec()),
            "/css/site.css" => ("text/css", br#"@import "base.css"; body { background: url(../img/bg.png) }"#.to_vec()),
            "/css/base.css" => ("text/css", b"p { color: red }".to_vec()),
            "/img/a.png" | "/img/bg.png" => ("image/png", vec![7u8; 64]),
            _ => return None,
        })
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn mirrors_a_site_with_working_links() {
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

        let started = crawl(&downloads, &jobs, root.clone(), request(&address, CrawlMode::Mirror)).unwrap();
        while ended.lock().unwrap().is_empty() {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        assert_eq!(ended.lock().unwrap()[0], JobResult::Done);
        assert_eq!(started.open.as_deref(), Some("Sites/127.0.0.1/index.html"));
        let mirror = root.join(&started.folder);
        let read = |path: &str| std::fs::read_to_string(mirror.join(path)).unwrap();
        assert!(read("index.html").contains(r#"href="css/site.css""#));
        assert!(read("index.html").contains(r#"href="blog/post/index.html""#));
        assert!(read("index.html").contains(r#"src="img/a.png""#));
        assert!(read("blog/post/index.html").contains(r#"href="../../css/site.css""#));
        assert!(read("blog/post/index.html").contains(r#"href="../../index.html""#));
        assert_eq!(read("css/site.css"), r#"@import "base.css"; body { background: url(../img/bg.png) }"#);
        assert!(mirror.join("css/base.css").is_file() && mirror.join("img/bg.png").is_file());

        let query = GalleryQuery { kind: Some("site".into()), site: None, search: None, sort: GallerySort::Newest, offset: 0, limit: 10 };
        let rows = library::page(&root, &query).unwrap();
        assert_eq!(rows.items.iter().map(|item| item.path.as_str()).collect::<Vec<_>>(), ["Sites/127.0.0.1/index.html"]);
    }
}

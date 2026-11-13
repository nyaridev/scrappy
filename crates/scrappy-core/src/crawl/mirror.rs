//! Mirror: a site saved for offline use (ADR-0008, ADR-0033). The crawler walks it within its limits; each page goes
//! into `Sites/<domain>/` with the stylesheets (and what they use), scripts and icons it needs, one local path per
//! address, and the images and videos it shows go into the mirror's record to pick from (`picks.rs`). Then every saved
//! page and stylesheet is rewritten to point at the copies, and at the live site for everything else.

// ── Imports ───────────────────────────────
use std::collections::{HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::time::Duration;

use reqwest::header::CONTENT_TYPE;
use reqwest::{Client, Url};
use tokio::sync::broadcast::error::RecvError;
use tokio::task::JoinSet;

use super::links::{asset_links, css_links, media_links};
use super::local::local_path;
use super::mirrors::{Mirror, Mirrors};
use super::request::{Plan, pages, website};
use super::rewrite::{Saved, rewrite_css, rewrite_html};
use crate::grab::{Find, Kept};
use crate::jobs::Job;
use crate::library::{self, Record};
use crate::net;

// ── Constants ─────────────────────────────
/// Files fetched at once.
const AT_ONCE: usize = 4;
/// A page's assets are held in memory before they're written, so huge ones are skipped; picked media streams.
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

/// A saved file: its address and where it went.
type SavedFile = (String, PathBuf);

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

/// Fetches `url` into the mirror at `dir`. A file other than a stylesheet that an earlier mirror saved is kept as it
/// is; a stylesheet is fetched again, as the saved one has its links rewritten.
async fn fetch(client: Client, dir: PathBuf, host: String, url: Url) -> Result<Fetched, String> {
    let local = local_path(&url, &host, false);
    let named_css = url.path().to_ascii_lowercase().ends_with(".css");
    if !named_css && let Ok(meta) = tokio::fs::metadata(dir.join(&local)).await {
        return Ok(Fetched { url, local, bytes: meta.len(), css: None });
    }
    let failed = |e: reqwest::Error| format!("A mirror file didn't download: {}", e.without_url());
    let response = client.get(url.clone()).send().await.and_then(|r| r.error_for_status()).map_err(failed)?;
    let too_big = || "A mirror file is too big to keep.".to_string();
    if response.content_length().is_some_and(|length| length > MAX_FILE_BYTES) {
        return Err(too_big());
    }
    let css = named_css
        || response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| value.starts_with("text/css"));
    let body = response.bytes().await.map_err(failed)?;
    if body.len() as u64 > MAX_FILE_BYTES {
        return Err(too_big());
    }
    write(&dir, &local, &body).await?;
    let css = css.then(|| css_links(&url, &String::from_utf8_lossy(&body)));
    Ok(Fetched { url, local, bytes: body.len() as u64, css })
}

/// Queues the links not seen yet.
fn queue(links: Vec<String>, seen: &mut HashSet<String>, waiting: &mut VecDeque<Url>) {
    for link in links.iter().filter_map(|link| key(link)) {
        if seen.insert(link.clone())
            && let Ok(url) = Url::parse(&link)
        {
            waiting.push_back(url);
        }
    }
}

/// The page's images and videos not listed yet, each with its copy when an earlier mirror saved it.
fn new_media(
    address: &Url,
    html: &str,
    dir: &Path,
    host: &str,
    listed: &mut HashSet<String>,
) -> Vec<(Find, Option<PathBuf>)> {
    media_links(address, html)
        .into_iter()
        .filter_map(|(link, kind)| {
            let link = key(&link)?;
            let find = Find::new(&link, kind)?;
            let local = local_path(&Url::parse(&link).ok()?, host, false);
            listed.insert(link).then(|| (find, dir.join(&local).is_file().then_some(local)))
        })
        .collect()
}

fn listed(files: Vec<(Url, PathBuf)>) -> Vec<SavedFile> {
    files.into_iter().map(|(url, local)| (url.to_string(), local)).collect()
}

/// The crawl itself: `run` without making the mirror idle when it ends.
async fn crawl(root: &Path, plan: &Plan, job: &Job, mirrors: &Mirrors) -> Result<u64, String> {
    let dir = root.join(&plan.folder);
    let host = plan.url.host_str().unwrap_or_default().to_string();
    let client = net::client()
        .timeout(FILE_TIMEOUT)
        .build()
        .map_err(|e| format!("Can't start the mirror: {e}"))?;
    let site = plan.site.as_str();
    let (url, folder) = (plan.url.to_string(), plan.folder.clone());
    mirrors.put(site, Mirror { url, folder, host: host.clone(), busy: true, ..Mirror::default() });
    let mut website = website(plan);
    let mut found = website.subscribe(64);
    let crawl = tokio::spawn(async move { website.crawl().await });

    let mut saved = Saved::new();
    let mut page_files: Vec<(Url, PathBuf)> = Vec::new();
    let mut sheets: Vec<(Url, PathBuf)> = Vec::new();
    let (mut seen, mut met) = (HashSet::new(), HashSet::new());
    let mut waiting = VecDeque::new();
    let mut fetches = JoinSet::new();
    let (mut files, mut bytes, mut crawling) = (0u64, 0u64, true);
    let report = |page_count: usize, files: u64| job.items(files, None, Some(pages(page_count)));
    loop {
        if job.cancelled() {
            crawl.abort();
            fetches.abort_all();
            break;
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
                    queue(asset_links(&address, &html), &mut seen, &mut waiting);
                    let media = new_media(&address, &html, &dir, &host, &mut met);
                    for (find, copy) in &media {
                        if let Some(local) = copy {
                            saved.insert(find.url.clone(), local.clone());
                        }
                    }
                    mirrors.update(site, |mirror| {
                        for (find, copy) in media {
                            if copy.is_some() {
                                mirror.kept.insert(find.url.clone(), Kept::Saved);
                            }
                            mirror.media.push(find);
                        }
                    });
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
                            queue(links, &mut seen, &mut waiting);
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
        return Err(if job.cancelled() {
            "Cancelled.".into()
        } else {
            "The site gave no pages. It may be down, or its robots.txt may not allow crawling.".into()
        });
    };
    job.items(files, None, Some("Updating links".into()));
    let (page_files, sheets) = (listed(page_files), listed(sheets));
    let rewrite = (dir.clone(), page_files.clone(), sheets.clone(), saved.clone());
    tokio::task::spawn_blocking(move || rewrite_all(&rewrite.0, &rewrite.1, &rewrite.2, &rewrite.3, false))
        .await
        .map_err(|_| "The link update stopped.".to_string())?;
    mirrors.update(site, |mirror| (mirror.pages, mirror.sheets, mirror.saved) = (page_files, sheets, saved));
    mirrors.store(root, site)?;
    library::record(
        root,
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
    if job.cancelled() {
        return Err("Cancelled.".into());
    }
    tracing::info!("A mirror saved {files} files");
    Ok(files)
}

// ── Public API ────────────────────────────
/// Where a mirror of `url` keeps its start page, relative to the mirror's folder.
pub(super) fn start_page(url: &Url) -> PathBuf {
    local_path(url, url.host_str().unwrap_or_default(), true)
}

/// Points the mirror at `dir`'s pages and stylesheets at the copies in `saved`. The first time (`again` false) their
/// links are read from their own addresses, and links to anything not saved go to the live site; `again`, only those
/// live links change. A stylesheet that isn't UTF-8 stays as it was.
pub(super) fn rewrite_all(dir: &Path, pages: &[SavedFile], sheets: &[SavedFile], saved: &Saved, again: bool) {
    let files = pages.iter().map(|(url, local)| (url, local, true)).chain(sheets.iter().map(|(url, local)| (url, local, false)));
    for (url, local, page) in files {
        let Ok(url) = Url::parse(url) else { continue };
        let base = (!again).then_some(&url);
        let file = dir.join(local);
        let Ok(text) = std::fs::read_to_string(&file) else { continue };
        let text = if page { rewrite_html(&text, base, local, saved) } else { rewrite_css(&text, base, local, saved) };
        if let Err(error) = std::fs::write(&file, text) {
            tracing::warn!("A mirror file wasn't rewritten: {error}");
        }
    }
}

/// Mirrors the site until it, the page limit or the job runs out. Progress reports the files saved, with the pages
/// saved as its message; the media its pages show go into `mirrors` as they're met. Ends with the mirror's record kept
/// and its start page in the catalog as one `site` row; a stopped mirror keeps what it saved.
pub(super) async fn run(root: PathBuf, plan: Plan, job: Job, mirrors: Mirrors) -> Result<u64, String> {
    let result = crawl(&root, &plan, &job, &mirrors).await;
    mirrors.update(&plan.site, |mirror| mirror.busy = false);
    result
}

#[cfg(test)]
mod tests {
    use crate::crawl::test_site::{request, serve};
    use crate::crawl::{CrawlMode, MediaRequest, Mirrors, crawl, save_media};
    use crate::grab::Kept;
    use crate::jobs::{JobEvent, JobResult, Jobs};
    use crate::library::{self, GalleryQuery, GallerySort};
    use crate::media::Downloads;
    use crate::tools::Toolbox;
    use crate::vault::{KeySource, Vault};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    /// Two pages sharing a stylesheet that imports another and uses a picture; the first shows a picture and a video.
    fn route(path: &str, _: &str) -> Option<(&'static str, Vec<u8>)> {
        Some(match path {
            "/" => (
                "text/html",
                br#"<link rel="stylesheet" href="/css/site.css"><a href="/blog/post">post</a><img src="/img/a.png">
                    <video src="/clip.mp4"></video>"#
                    .to_vec(),
            ),
            "/blog/post" => ("text/html", br#"<link rel="stylesheet" href="/css/site.css"><a href="/">home</a>"#.to_vec()),
            "/css/site.css" => ("text/css", br#"@import "base.css"; body { background: url(../img/bg.png) }"#.to_vec()),
            "/css/base.css" => ("text/css", b"p { color: red }".to_vec()),
            "/img/a.png" | "/img/bg.png" => ("image/png", vec![7u8; 64]),
            "/clip.mp4" => ("video/mp4", vec![1u8; 2 * 1024 * 1024]),
            _ => return None,
        })
    }

    /// Waits until `ended` has `count` results.
    async fn ends(ended: &Mutex<Vec<JobResult>>, count: usize) {
        while ended.lock().unwrap().len() < count {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn mirrors_a_site_then_saves_picked_media() {
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
        let mirrors = Mirrors::default();

        let started = crawl(&downloads, &jobs, &mirrors, root.clone(), request(&address, CrawlMode::Mirror)).unwrap();
        ends(&ended, 1).await;
        assert_eq!(ended.lock().unwrap()[0], JobResult::Done);
        assert_eq!(started.open.as_deref(), Some("Sites/127.0.0.1/index.html"));
        let mirror = root.join(&started.folder);
        let read = |path: &str| std::fs::read_to_string(mirror.join(path)).unwrap();
        assert!(read("index.html").contains(r#"href="css/site.css""#));
        assert!(read("index.html").contains(r#"href="blog/post/index.html""#));
        // Media is listed, not saved: the page shows it from the live site.
        assert!(read("index.html").contains(&format!(r#"src="{address}img/a.png""#)), "{}", read("index.html"));
        assert!(!mirror.join("img/a.png").exists());
        assert!(read("blog/post/index.html").contains(r#"href="../../css/site.css""#));
        assert!(read("blog/post/index.html").contains(r#"href="../../index.html""#));
        assert_eq!(read("css/site.css"), r#"@import "base.css"; body { background: url(../img/bg.png) }"#);
        assert!(mirror.join("css/base.css").is_file() && mirror.join("img/bg.png").is_file());
        let query = GalleryQuery { kind: Some("site".into()), folder: None, search: None, sort: GallerySort::Newest, offset: 0, limit: 10 };
        let rows = library::page(&root, &library::Disk::new(Arc::new(|| {})), &query).unwrap();
        assert_eq!(rows.items.iter().map(|item| item.path.as_str()).collect::<Vec<_>>(), ["Sites/127.0.0.1/index.html"]);

        // Read back as after a restart, then the picture and the video (over the 1 MB most) are picked.
        let media = Mirrors::default().media(&root, &started.site).unwrap();
        let names: Vec<&str> = media.finds.iter().map(|find| find.name.as_str()).collect();
        assert_eq!(names, ["clip.mp4", "a.png"]);
        let (picture, video) = (format!("{address}img/a.png"), format!("{address}clip.mp4"));
        let urls = vec![picture.clone(), video.clone(), "https://elsewhere.org/x.png".into()];
        let picked = MediaRequest { site: started.site.clone(), urls, max_video_mb: 1 };
        save_media(&mirrors, &downloads, &jobs, root.clone(), picked).unwrap();
        ends(&ended, 2).await;
        assert_eq!(ended.lock().unwrap()[1], JobResult::Done);
        assert!(read("index.html").contains(r#"<img src="img/a.png">"#), "{}", read("index.html"));
        assert!(read("index.html").contains(&format!(r#"src="{video}""#)));
        assert!(mirror.join("img/a.png").is_file() && !mirror.join("clip.mp4").exists());
        let media = mirrors.media(&root, &started.site).unwrap();
        assert_eq!((media.kept[&picture].clone(), media.kept[&video].clone()), (Kept::Saved, Kept::Big));
        assert!(!media.busy && !media.kept.contains_key("https://elsewhere.org/x.png"));
    }
}

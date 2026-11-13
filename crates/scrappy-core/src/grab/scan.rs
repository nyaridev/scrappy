//! Scan: Grab's deep scan. Opens the page in the installed Chrome, Brave or Edge, headless and with a fresh profile
//! (never the user's: no cookies, no history), records every response until the page goes quiet, plays the page's
//! videos so their streams load, then adds the page's links to files (ADR-0007). Past the first page it follows the
//! page's links on its site, level by level, a few tabs at once, up to a depth (ADR-0029, ADR-0032). It can save its
//! finds as it finds them (`keep.rs`, ADR-0031). It runs as a job, and keeps what it has found so far for the window
//! to read while it runs (ADR-0019).

// ── Imports ───────────────────────────────
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use chromiumoxide::browser::{Browser, BrowserConfig};
use chromiumoxide::cdp::browser_protocol::network::{EnableParams, EventResponseReceived};
use futures_util::{StreamExt, stream};
use reqwest::Url;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use super::find::{Find, FindKind, Found, Seen, classify};
use super::keep::{Kept, Keeper, Wanted};
use crate::jobs::{Job, JobId, Jobs};
use crate::media::{DownloadSource, Downloads, Row};
use crate::{net, tools, vault};

// ── Constants ─────────────────────────────
const LAUNCH_TIMEOUT: Duration = Duration::from_secs(30);
const LOAD_TIMEOUT: Duration = Duration::from_secs(30);
/// The longest a page runs after it loads, so players start and lazy images arrive.
const SETTLE: Duration = Duration::from_secs(8);
/// A page that has had no new response for this long is done sooner (ADR-0031).
const QUIET: Duration = Duration::from_millis(1500);
/// How often what the scan has seen is sorted into finds for the window.
const SHOW_EVERY: Duration = Duration::from_millis(500);
/// How often a wait checks whether the scan was stopped.
const STOP_CHECK: Duration = Duration::from_millis(200);
/// The deepest scan, and the most pages it looks at (ADR-0032).
const MAX_DEPTH: u32 = 10;
const MAX_PAGES: u32 = 10_000;
const MAX_FILES: u32 = 10_000;
/// Pages past the first looked at at once, each in its own tab.
const TABS: usize = 6;
/// Starts every video and audio element, muted, so the page requests its stream.
const PLAY: &str = "document.querySelectorAll('video, audio').forEach((media) => { media.muted = true; \
                    media.play().catch(() => {}); })";
/// Every link on the page that could be a file: images (with `srcset`), media sources and anchors.
const LINKS: &str = r#"(() => {
    const urls = new Set();
    for (const el of document.querySelectorAll("img, video, audio, source, a[href]")) {
        for (const url of [el.currentSrc, el.src, el.href]) if (typeof url === "string") urls.add(url);
        for (const part of (el.srcset || "").split(",")) {
            const url = part.trim().split(/\s+/)[0];
            if (url) urls.add(new URL(url, location.href).href);
        }
    }
    return [...urls].filter((url) => url.startsWith("http")).slice(0, 5000);
})()"#;
/// The page's links to other pages: where a deeper scan goes next.
const PAGE_LINKS: &str = r#"[...document.querySelectorAll("a[href]")]
    .map((a) => a.href)
    .filter((url) => url.startsWith("http"))
    .slice(0, 2000)"#;

// ── Types ─────────────────────────────────
/// What to scan: a page, how far past it (ADR-0029), and whether to save finds as they're found (ADR-0031).
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ScanRequest {
    pub url: String,
    /// 1: the page alone; 2: also the pages it links to on its site; and so on.
    pub depth: u32,
    /// Save each find of `kinds` as it's found.
    pub save: bool,
    pub kinds: Vec<FindKind>,
    /// Files under this many kilobytes aren't kept.
    pub min_kb: u32,
    /// The most files saved.
    pub files: u32,
    /// Once the scan ends, move the folder's extra copies to the trash.
    pub remove_duplicates: bool,
}

/// What a scan found on a page.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PageScan {
    /// The page's address once it loaded (after redirects).
    pub url: String,
    pub title: Option<String>,
    pub finds: Vec<Find>,
    /// How many pages the scan has looked at (ADR-0029).
    pub pages: u32,
    /// With saving as found on: each kept find's state, by link (ADR-0031).
    #[serde(default)]
    pub kept: HashMap<String, Kept>,
}

type Pages = Arc<Mutex<HashMap<JobId, PageScan>>>;

/// One page looked at: its title, its address once loaded, and its links to other pages.
struct Visit {
    title: Option<String>,
    address: String,
    links: Vec<String>,
}

/// Running and finished scans: what each has found so far. Only the latest is kept. Clones share it.
#[derive(Clone, Default)]
pub struct Scans {
    pages: Pages,
}

// ── Private helpers ───────────────────────
/// A response's size: the total of a range answer (`bytes 0-1/1234`), else its length.
fn size_of(headers: &Value) -> Option<u64> {
    let header = |name: &str| {
        headers.as_object()?.iter().find(|(key, _)| key.eq_ignore_ascii_case(name)).and_then(|(_, value)| value.as_str())
    };
    let total = header("content-range").and_then(|range| range.rsplit_once('/')).and_then(|(_, total)| total.parse().ok());
    total.or_else(|| header("content-length").and_then(|length| length.trim().parse().ok()))
}

fn failed(what: &str) -> impl Fn(chromiumoxide::error::CdpError) -> String + '_ {
    move |error| format!("The browser couldn't {what}: {error}")
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The request with an http or https link and every limit in range.
fn checked(request: ScanRequest) -> Result<ScanRequest, String> {
    let url = Url::parse(request.url.trim()).map_err(|_| "Not a link.".to_string())?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err("Only http and https links work.".into());
    }
    let kinds: Vec<FindKind> = request.kinds.into_iter().filter(|kind| *kind != FindKind::Stream).collect();
    if request.save && kinds.is_empty() {
        return Err("Pick at least one kind of file to keep.".into());
    }
    Ok(ScanRequest {
        url: url.into(),
        depth: request.depth.clamp(1, MAX_DEPTH),
        files: request.files.clamp(1, MAX_FILES),
        kinds,
        ..request
    })
}

/// Puts the finds so far where the window reads them and reports: the finds, or with saving on, the files saved.
fn show(pages: &Pages, job: &Job, finds: Vec<Find>, looked: u32, keeper: Option<&Keeper>) {
    job.items(keeper.map_or(finds.len() as u64, Keeper::saved), None, None);
    if let Some(page) = lock(pages).get_mut(&job.id()) {
        page.finds = finds;
        page.pages = looked;
        page.kept = keeper.map(Keeper::states).unwrap_or_default();
    }
}

/// The links in `found` to pages on `site` that aren't files and weren't looked at yet, each once, without their
/// `#part`. Each is added to `visited`.
fn new_pages(
    found: impl IntoIterator<Item = String>,
    site: Option<&str>,
    visited: &mut HashSet<String>,
) -> Vec<String> {
    found
        .into_iter()
        .filter_map(|link| {
            let mut url = Url::parse(&link).ok()?;
            url.set_fragment(None);
            let link = String::from(url);
            let fresh = classify(&link, None).is_none() && vault::site_of(&link).as_deref() == site;
            (fresh && visited.insert(link.clone())).then_some(link)
        })
        .collect()
}

/// Waits until `last` (a page's latest response) is `QUIET` old, at most `SETTLE`, or less once `job` is stopped.
async fn settle(job: &Job, last: &Mutex<Instant>) {
    let end = Instant::now() + SETTLE;
    while !job.cancelled() && Instant::now() < end && lock(last).elapsed() < QUIET {
        tokio::time::sleep(STOP_CHECK).await;
    }
}

/// Waits `time`, or less once `job` is stopped.
async fn wait(job: &Job, time: Duration) {
    let end = Instant::now() + time;
    while !job.cancelled() {
        let left = end.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return;
        }
        tokio::time::sleep(left.min(STOP_CHECK)).await;
    }
}

/// Scans `request` (already checked) with the browser at `program`, keeping what it finds in `pages` as it goes and
/// handing each find to `keeper` when it saves as found.
async fn scan_with(
    program: &Path,
    request: &ScanRequest,
    job: &Job,
    pages: &Pages,
    keeper: Option<&Keeper>,
) -> Result<PageScan, String> {
    let profile = tempfile::Builder::new().prefix("scrappy-scan-").tempdir().map_err(|e| format!("Can't scan: {e}"))?;
    let config = BrowserConfig::builder()
        .chrome_executable(program)
        .user_data_dir(profile.path())
        .new_headless_mode()
        .launch_timeout(LAUNCH_TIMEOUT)
        .args(["--mute-audio", "--no-first-run", "--no-default-browser-check", "--autoplay-policy=no-user-gesture-required"])
        // Chrome names `socks5h` (names resolved by the proxy) `socks5`, which it always resolves there.
        .args(net::proxy().map(|proxy| format!("--proxy-server={}", proxy.replacen("socks5h://", "socks5://", 1))))
        .build()?;
    let (mut browser, mut handler) = Browser::launch(config).await.map_err(failed("start"))?;
    let events = tokio::spawn(async move { while handler.next().await.is_some() {} });
    let result = run(&browser, request, job, pages, keeper).await;
    let _ = browser.close().await;
    let _ = browser.wait().await;
    events.abort();
    result
}

/// Looks at one page in a new tab: every response it gets goes into `seen` until the page goes quiet, then its links
/// to files. `title_to`, for the first page, gets its title as soon as it loads. Stopped, it ends early with what it
/// saw.
async fn scan_page(
    browser: &Browser,
    url: &str,
    job: &Job,
    seen: &Arc<Mutex<Found>>,
    title_to: Option<&Pages>,
) -> Result<Visit, String> {
    let page = browser.new_page("about:blank").await.map_err(failed("open a tab"))?;
    // Headless Chrome names itself in its user agent, and some sites refuse it; the plain browser's name is used.
    if let Ok(version) = browser.version().await {
        let _ = page.set_user_agent(version.user_agent.replace("HeadlessChrome", "Chrome")).await;
    }
    page.execute(EnableParams::default()).await.map_err(failed("watch the page"))?;
    let mut responses = page.event_listener::<EventResponseReceived>().await.map_err(failed("watch the page"))?;
    let (sink, last) = (seen.clone(), Arc::new(Mutex::new(Instant::now())));
    let heard = last.clone();
    let listen = tokio::spawn(async move {
        while let Some(event) = responses.next().await {
            let response = &event.response;
            let seen = Seen {
                url: response.url.clone(),
                mime: Some(response.mime_type.clone()).filter(|mime| !mime.is_empty()),
                size: size_of(response.headers.inner()),
            };
            lock(&sink).add(seen);
            *lock(&heard) = Instant::now();
        }
    });

    // A page that never finishes loading still gets scanned with what it loaded.
    tokio::select! {
        _ = tokio::time::timeout(LOAD_TIMEOUT, page.goto(url)) => {}
        () = wait(job, LOAD_TIMEOUT) => {}
    }
    let title = page.get_title().await.ok().flatten().filter(|title| !title.trim().is_empty());
    if let Some(pages) = title_to
        && let Some(shown) = lock(pages).get_mut(&job.id())
    {
        shown.title.clone_from(&title);
    }
    if !job.cancelled() {
        let _ = page.evaluate(PLAY).await;
        *lock(&last) = Instant::now();
        settle(job, &last).await;
    }
    let read = |script: &'static str| {
        let page = &page;
        async move {
            let result = page.evaluate(script).await.ok();
            result.and_then(|result| result.into_value::<Vec<String>>().ok()).unwrap_or_default()
        }
    };
    let (files, links) = (read(LINKS).await, read(PAGE_LINKS).await);
    listen.abort();
    let address = page.url().await.ok().flatten().unwrap_or_else(|| url.to_string());
    {
        let mut found = lock(seen);
        files.into_iter().for_each(|url| found.add(Seen { url, ..Seen::default() }));
    }
    let _ = page.close().await;
    Ok(Visit { title, address, links })
}

/// Looks at the first page, then at its site's pages it links to, level by level up to the depth, `TABS` at a time,
/// until the pages run out or `MAX_PAGES` were looked at (ADR-0029, ADR-0032). Every page's finds go in one list, and to `keeper` when it saves them as found.
/// Stopped, it ends early with what it saw.
async fn run(
    browser: &Browser,
    request: &ScanRequest,
    job: &Job,
    pages: &Pages,
    keeper: Option<&Keeper>,
) -> Result<PageScan, String> {
    let seen: Arc<Mutex<Found>> = Arc::default();
    let looked = Arc::new(AtomicU32::new(0));
    // Every find goes to the keeper, not only those the window lists. A browser's finds aren't tied to one page: the
    // scanned page is their Referer.
    let offer = |found: &Found, keeper: Option<&Keeper>, page: &str, job: &Job| {
        if let Some(keeper) = keeper {
            found.all().for_each(|find| keeper.offer(&find.url, find.kind, page, job));
        }
    };
    let show = {
        let (seen, pages, job, looked) = (seen.clone(), pages.clone(), job.clone(), looked.clone());
        let (keeper, page) = (keeper.cloned(), request.url.clone());
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(SHOW_EVERY).await;
                let finds = {
                    let found = lock(&seen);
                    offer(&found, keeper.as_ref(), &page, &job);
                    found.finds()
                };
                show(&pages, &job, finds, looked.load(Ordering::Relaxed), keeper.as_ref());
            }
        })
    };
    let first = scan_page(browser, &request.url, job, &seen, Some(pages)).await;
    looked.fetch_add(1, Ordering::Relaxed);
    let first = match first {
        Ok(first) => first,
        Err(error) => {
            show.abort();
            return Err(error);
        }
    };
    let site = vault::site_of(&first.address);
    let mut visited: HashSet<String> = HashSet::new();
    new_pages([request.url.clone(), first.address.clone()], site.as_deref(), &mut visited);
    let mut level = new_pages(first.links, site.as_deref(), &mut visited);
    let mut left = MAX_PAGES - 1;
    for _ in 1..request.depth {
        if job.cancelled() || left == 0 || level.is_empty() {
            break;
        }
        level.truncate(left as usize);
        left -= level.len() as u32;
        // Collected first: a stream over borrowed pages needs its futures made up front.
        let visits: Vec<_> = level
            .iter()
            .map(|url| {
                let (seen, looked) = (&seen, &looked);
                async move {
                    let visit = scan_page(browser, url, job, seen, None).await;
                    looked.fetch_add(1, Ordering::Relaxed);
                    visit
                }
            })
            .collect();
        let visits: Vec<Result<Visit, String>> = stream::iter(visits).buffer_unordered(TABS).collect().await;
        level = new_pages(visits.into_iter().flatten().flat_map(|visit| visit.links), site.as_deref(), &mut visited);
    }
    show.abort();
    let found = std::mem::take(&mut *lock(&seen));
    offer(&found, keeper, &request.url, job);
    let finds = found.finds();
    let looked = looked.load(Ordering::Relaxed);
    tracing::info!("A deep scan found {} things on {looked} pages", finds.len());
    Ok(PageScan { url: first.address, title: first.title, finds, pages: looked, kept: HashMap::new() })
}

// ── Public API ────────────────────────────
impl Scans {
    /// Starts a scan of `request` as a job with the installed Chrome, Brave or Edge, dropping earlier scans. Its
    /// progress is `items`: the finds so far, or with saving as found on, the files saved; `page`
    /// returns what it found. Saving as found saves into the library at `root` and gets a row in the Download list.
    /// Must be called inside a Tokio runtime.
    pub fn start(
        &self,
        jobs: &Arc<Jobs>,
        downloads: &Downloads,
        root: Option<PathBuf>,
        request: ScanRequest,
    ) -> Result<JobId, String> {
        let request = checked(request)?;
        let program = tools::browser().ok_or("Scanning needs Chrome, Brave or Edge installed.")?;
        let keeper = if request.save {
            let root = root.ok_or("Set up the library first.")?;
            let (kinds, min_bytes) = (request.kinds.clone(), u64::from(request.min_kb) * 1024);
            Some(Keeper::new(downloads.clone(), root, &request.url, Wanted { kinds, min_bytes, files: request.files })?)
        } else {
            None
        };
        let saving = keeper.as_ref().map(|keeper| (keeper.folder(), vault::site_of(&request.url)));
        let pages = self.pages.clone();
        let (url, remove_duplicates) = (request.url.clone(), request.remove_duplicates);
        let work = move |job: Job| async move {
            {
                let mut all = lock(&pages);
                all.clear();
                let (url, kept) = (request.url.clone(), HashMap::new());
                all.insert(job.id(), PageScan { url, title: None, finds: Vec::new(), pages: 0, kept });
            }
            let found = scan_with(&program, &request, &job, &pages, keeper.as_ref()).await;
            let saved = match &keeper {
                Some(keeper) => keeper.finish(&job, remove_duplicates).await?,
                None => 0,
            };
            let mut found = found?;
            found.kept = keeper.as_ref().map(Keeper::states).unwrap_or_default();
            lock(&pages).insert(job.id(), found);
            Ok(saved)
        };
        Ok(match saving {
            Some((folder, site)) => {
                let title = site.unwrap_or_else(|| url.clone());
                let row = Row { source: DownloadSource::Grab, url, title, kind: "Files".into(), thumbnail: None };
                downloads.track(jobs, row, folder, work)
            }
            None => jobs.start(move |job| async move { work(job).await.map(|_| ()) }),
        })
    }

    /// What the scan `id` has found so far; all of it once its job has ended. `None` once a newer scan started.
    pub fn page(&self, id: JobId) -> Option<PageScan> {
        lock(&self.pages).get(&id).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::{ScanRequest, Scans, checked, new_pages, size_of};
    use crate::grab::{FindKind, PageScan};
    use crate::jobs::{JobEvent, Jobs};
    use crate::media::Downloads;
    use crate::tools::Toolbox;
    use crate::vault::{KeySource, Vault};
    use serde_json::json;
    use std::sync::Arc;
    use std::time::Duration;

    /// A browser scan of `url` that saves nothing.
    fn browser(url: &str, depth: u32) -> ScanRequest {
        ScanRequest {
            url: url.into(),
            depth,
            save: false,
            kinds: Vec::new(),
            min_kb: 0,
            files: 1,
            remove_duplicates: false,
        }
    }

    #[test]
    fn keeps_the_limits() {
        assert_eq!(checked(browser("https://a.com/", 99)).unwrap().depth, 10);
        assert_eq!(checked(browser("https://a.com/", 0)).unwrap().depth, 1);
        let streams = ScanRequest { save: true, kinds: vec![FindKind::Stream], ..browser("https://a.com/", 1) };
        assert!(checked(streams).unwrap_err().contains("Pick"));
    }

    /// Runs a scan of `url` alone as a job and returns its page once it ends.
    async fn scan(url: &str) -> Result<PageScan, String> {
        scan_deep(browser(url, 1), u32::MAX).await
    }

    /// Runs a scan as a job, stops it once it has looked at `pages` pages, and returns its page once it ends.
    async fn scan_deep(request: ScanRequest, pages: u32) -> Result<PageScan, String> {
        let (send, mut ended) = tokio::sync::mpsc::unbounded_channel();
        let jobs = Jobs::new(Arc::new(move |event| {
            if let JobEvent::Finished(finished) = event {
                let _ = send.send(finished);
            }
        }));
        let dir = tempfile::tempdir().unwrap();
        let vault = Vault::open(&dir.path().join("vault"), KeySource::Fixed([1; 32]));
        let downloads = Downloads::new(Toolbox::new(dir.path().join("tools")), vault);
        let scans = Scans::default();
        let id = scans.start(&jobs, &downloads, None, request)?;
        loop {
            tokio::select! {
                _ = ended.recv() => break,
                () = tokio::time::sleep(Duration::from_millis(100)) => {
                    if scans.page(id).is_some_and(|page| page.pages >= pages) {
                        jobs.cancel(id);
                    }
                }
            }
        }
        Ok(scans.page(id).unwrap())
    }

    /// Scans two public pages with this PC's browser: run with `cargo test -p scrappy-core -- --ignored`.
    #[tokio::test]
    #[ignore = "needs Chrome, Brave or Edge, and the network"]
    async fn finds_a_video_and_a_stream() {
        let page = scan("https://www.w3schools.com/html/html5_video.asp").await.unwrap();
        assert!(page.title.is_some_and(|title| title.contains("Video")));
        let finds = page.finds;
        assert!(finds.iter().any(|f| f.kind == FindKind::Video && f.name.ends_with(".mp4")), "{finds:#?}");
        let finds = scan("https://hls-js.netlify.app/demo/").await.unwrap().finds;
        assert!(scan("file:///C:/secret").await.unwrap_err().contains("http"));
        assert!(finds.iter().any(|f| f.kind == FindKind::Stream), "{finds:#?}");
    }

    /// Follows a page's links on its site (ADR-0029): run with `cargo test -p scrappy-core -- --ignored`.
    #[tokio::test]
    #[ignore = "needs Chrome, Brave or Edge, and the network"]
    async fn follows_linked_pages() {
        let request = browser("https://www.w3schools.com/html/html5_video.asp", 2);
        let page = scan_deep(request, 3).await.unwrap();
        assert!(page.pages >= 3, "{}", page.pages);
        assert!(page.finds.iter().any(|f| f.kind == FindKind::Video), "{:#?}", page.finds);
    }

    #[test]
    fn follows_only_new_pages_on_the_site() {
        let mut visited = std::collections::HashSet::new();
        let links = [
            "https://www.a.com/gallery#top",
            "https://a.com/gallery",
            "https://cdn.a.com/page/2",
            "https://a.com/photo.jpg",
            "https://b.com/elsewhere",
            "mailto:me@a.com",
        ]
        .map(String::from);
        let found = new_pages(links.clone(), Some("a.com"), &mut visited);
        assert_eq!(found, ["https://www.a.com/gallery", "https://a.com/gallery", "https://cdn.a.com/page/2"]);
        assert!(new_pages(links, Some("a.com"), &mut visited).is_empty());
    }

    #[test]
    fn reads_a_responses_size() {
        assert_eq!(size_of(&json!({"Content-Length": "2048"})), Some(2048));
        assert_eq!(size_of(&json!({"content-range": "bytes 0-1023/99999", "content-length": "1024"})), Some(99999));
        assert_eq!(size_of(&json!({"content-range": "bytes 0-1023/*"})), None);
        assert_eq!(size_of(&json!({})), None);
    }
}

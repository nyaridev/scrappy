//! Run: a hunt as one job. An engine source gives full-size images directly (ADR-0025). A site source takes the image
//! search results on its site, then reads their pages (ADR-0026); "Any site" reads the pages of a web search (ADR-0013).
//! Pages are read as plain HTTP and the content finder lists their images. Images are saved into `Images/<topic>/`
//! (web images through `media`, inline ones written here), skipping links the catalog already has. Status run four at
//! once, take turns to search each host, and each saves up to its own limit; their progress is in `status` (ADR-0024).
//! With "Remove duplicates" on, the hunt ends by moving its folder's extra copies to the trash (ADR-0027).

// ── Imports ───────────────────────────────
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use futures_util::{StreamExt, stream};
use reqwest::header::CONTENT_TYPE;
use reqwest::{Client, Url};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use ts_rs::TS;

use super::content::{self, ImageKind};
use super::engines::{self, Ask, Bases, Hit, ImageEngine};
use super::status::{self, HuntSource, HuntStatus, Hunts, SourceState, Status};
use super::{pack, search};
use crate::net;
use crate::dedupe;
use crate::jobs::{Job, JobId, Jobs};
use crate::library::{self, Record, template};
use crate::media::{DownloadSource, Downloads, Row, SaveRequest};
use crate::vault;

// ── Constants ─────────────────────────────
const IMAGES: &str = "Images";
/// What the web search source is called.
const ANY_SITE: &str = "Any site";
/// Status hunted at once.
const SOURCES_AT_ONCE: usize = 4;
/// Pages one source reads at once.
const PAGES_AT_ONCE: usize = 2;
/// Images one page saves at once.
const AT_ONCE: usize = 4;
/// Search results one source saves at once.
const IMAGES_AT_ONCE: usize = 8;
/// Requests of results each try of a site's image search makes.
const SITE_REQUESTS: usize = 2;
/// Search turns: one per host (`ImageEngine::turn`; the web search uses DuckDuckGo's).
const TURNS: usize = 3;
/// An engine source asks for this many times its limit: some images fail or are too small.
const SPARE: u32 = 2;
/// How often the job reports while it runs, so the window sees sources change even when nothing is saved.
const TICK: Duration = Duration::from_millis(500);
const MAX_QUERY: usize = 200;
const MAX_PAGES: u32 = 50;
const MAX_PER_SOURCE: u32 = 1000;
const MAX_PAGE_BYTES: usize = 5 * 1024 * 1024;
const MAX_INLINE_BYTES: usize = 10 * 1024 * 1024;
const PAGE_TIMEOUT: Duration = Duration::from_secs(30);

// ── Types ─────────────────────────────────
/// What to hunt: a topic, a pack and the parts of it picked, the kinds of image data, and the limits.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct HuntRequest {
    pub query: String,
    /// The pack's id.
    pub pack: String,
    /// Search any site (when the pack does).
    pub web: bool,
    /// The pack's sites to search.
    pub sites: Vec<String>,
    /// Image search engines to read.
    pub engines: Vec<ImageEngine>,
    /// What a site source's content finder takes; engines give whole images.
    pub kinds: Vec<ImageKind>,
    /// The most result pages read per source (an engine's requests of results, up to 10).
    pub pages: u32,
    /// The most images saved from each source.
    pub per_source: u32,
    /// Images under this many kilobytes aren't kept: icons, thumbnails.
    pub min_kb: u32,
    /// Once every source ended, move the folder's duplicates to the trash, keeping each picture's best copy.
    pub remove_duplicates: bool,
}

/// A hunt that started: its job, and the folder its images go to.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct HuntStarted {
    pub id: JobId,
    /// Relative to the library root.
    pub folder: String,
}

/// What one source searches.
#[derive(Debug)]
enum Search {
    /// Full-size images from an engine.
    Images(ImageEngine),
    /// A site: its image search results, then their pages.
    Site(String),
    /// Result pages of a web search, read with the content finder ("Any site").
    Pages(String),
}

/// What a source found: pages to read, or images to save.
enum Item {
    Page(String),
    Image(Hit),
}

/// A request that passed its checks.
#[derive(Debug)]
struct Plan {
    query: String,
    /// Each source: its name, and what it searches.
    searches: Vec<(String, Search)>,
    /// The web search page's address (a local one in tests).
    engine: String,
    /// Where the image search engines are (local ones in tests).
    bases: Bases,
    /// After each search, before the next source's (none in tests).
    pause: Duration,
    kinds: Vec<ImageKind>,
    folder: PathBuf,
    pages: u32,
    per_source: u32,
    min_bytes: u64,
    remove_duplicates: bool,
}

/// What a hunt's sources share while they run.
struct Shared<'a> {
    downloads: &'a Downloads,
    root: &'a Path,
    plan: &'a Plan,
    job: &'a Job,
    client: Client,
    /// Links the catalog already has.
    known: HashSet<String>,
    pages_seen: Mutex<HashSet<String>>,
    images_seen: Mutex<HashSet<String>>,
    /// One search at a time per host: engines refuse bursts.
    turns: [tokio::sync::Mutex<()>; TURNS],
    sources: Status,
}

// ── Private helpers ───────────────────────
/// Checks the request against the packs in `packs_dir` and keeps the limits in range.
fn plan(packs_dir: Option<&Path>, request: HuntRequest) -> Result<Plan, String> {
    let query = request.query.trim().to_string();
    if query.is_empty() || query.chars().count() > MAX_QUERY {
        return Err("Type a topic.".into());
    }
    let pack = pack::packs(packs_dir).packs.into_iter().find(|pack| pack.id == request.pack).ok_or("That pack isn't there anymore.")?;
    let mut searches = Vec::new();
    for engine in [ImageEngine::DuckDuckGo, ImageEngine::Bing, ImageEngine::Yandex] {
        if request.engines.contains(&engine) {
            searches.push((engine.name().to_string(), Search::Images(engine)));
        }
    }
    let engines = searches.len();
    if request.web && pack.web {
        searches.push((ANY_SITE.to_string(), Search::Pages(query.clone())));
    }
    for site in pack.sites.iter().filter(|site| request.sites.contains(site)) {
        searches.push((site.clone(), Search::Site(site.clone())));
    }
    if searches.is_empty() {
        return Err("Pick at least one place to search.".into());
    }
    if searches.len() > engines && request.kinds.is_empty() {
        return Err("Pick at least one kind of image.".into());
    }
    Ok(Plan {
        searches,
        engine: search::ENGINE.to_string(),
        bases: Bases::default(),
        pause: search::DELAY,
        kinds: request.kinds,
        folder: PathBuf::from(IMAGES).join(template::sanitize(&query)),
        query,
        pages: request.pages.clamp(1, MAX_PAGES),
        per_source: request.per_source.clamp(1, MAX_PER_SOURCE),
        min_bytes: u64::from(request.min_kb) * 1024,
        remove_duplicates: request.remove_duplicates,
    })
}

/// Every source of `plan`, waiting.
fn waiting(plan: &Plan) -> Status {
    let source = |(name, _): &(String, Search)| HuntSource {
        name: name.clone(),
        state: SourceState::Waiting,
        read: 0,
        found: 0,
        saved: 0,
        limit: plan.per_source,
        problem: None,
    };
    Arc::new(Mutex::new(HuntStatus { sources: plan.searches.iter().map(source).collect(), removing: false, removed: None }))
}

/// A result page's HTML, read as plain HTTP; anything that isn't a web page is skipped.
async fn fetch_page(client: &Client, url: &Url) -> Result<String, String> {
    let failed = |e: reqwest::Error| e.without_url().to_string();
    let response = client.get(url.clone()).send().await.and_then(|r| r.error_for_status()).map_err(failed)?;
    let html = response.headers().get(CONTENT_TYPE).and_then(|value| value.to_str().ok()).is_some_and(|kind| kind.contains("html"));
    if !html || response.content_length().is_some_and(|length| length > MAX_PAGE_BYTES as u64) {
        return Err("Not a web page, or too big.".into());
    }
    let body = response.bytes().await.map_err(failed)?;
    if body.len() > MAX_PAGE_BYTES {
        return Err("The page is too big.".into());
    }
    Ok(String::from_utf8_lossy(&body).into_owned())
}

/// A `data:` image's bytes and file extension; `None` for anything else.
fn decode_data(url: &str) -> Option<(Vec<u8>, &'static str)> {
    let (meta, payload) = url.strip_prefix("data:")?.split_once(',')?;
    let mime = meta.split(';').next()?.to_ascii_lowercase();
    let ext = match mime.as_str() {
        "image/png" => "png",
        "image/jpeg" | "image/jpg" => "jpg",
        "image/gif" => "gif",
        "image/webp" => "webp",
        "image/avif" => "avif",
        "image/bmp" => "bmp",
        "image/svg+xml" => "svg",
        _ => return None,
    };
    let bytes = if meta.ends_with(";base64") {
        STANDARD.decode(payload.chars().filter(|c| !c.is_whitespace()).collect::<String>()).ok()?
    } else {
        let mut url = Url::parse("http://x/").ok()?;
        url.set_query(Some(&format!("d={payload}")));
        url.query_pairs().next()?.1.into_owned().into_bytes()
    };
    Some((bytes, ext))
}

/// Writes an inline image into the hunt's folder, named by its content, and adds it to the catalog. False when it's
/// too small or too big, or the folder already has it.
async fn save_inline(root: &Path, plan: &Plan, page: &str, url: &str) -> Result<bool, String> {
    let Some((bytes, ext)) = decode_data(url) else { return Ok(false) };
    if (bytes.len() as u64) < plan.min_bytes || bytes.len() > MAX_INLINE_BYTES {
        return Ok(false);
    }
    let hash: String = format!("{:x}", Sha256::digest(&bytes)).chars().take(12).collect();
    let path = plan.folder.join(format!("inline-{hash}.{ext}"));
    let file = root.join(&path);
    if file.exists() {
        return Ok(false);
    }
    tokio::fs::create_dir_all(root.join(&plan.folder)).await.map_err(|e| format!("Can't make the hunt's folder: {e}"))?;
    tokio::fs::write(&file, &bytes).await.map_err(|e| format!("Can't save an inline image: {e}"))?;
    let size = bytes.len() as u64;
    let record = Record {
        path: path.to_string_lossy().replace('\\', "/"),
        kind: "image",
        source_url: page.to_string(),
        site: vault::site_of(page),
        user: None,
        size_bytes: size,
        duration_secs: None,
        codec: None,
        transferred_bytes: size,
    };
    library::record(root, &record).map(|()| true)
}

/// True the first time `item` is seen in `seen`.
fn first_time(seen: &Mutex<HashSet<String>>, item: &str) -> bool {
    seen.lock().unwrap_or_else(PoisonError::into_inner).insert(item.to_string())
}

/// Saves the image at `url` (found on `page`) into source `index`. It takes a slot of the source's limit (`taken`)
/// before it downloads and gives it back when it isn't kept, so the source never goes past its limit.
async fn save_one(shared: &Shared<'_>, index: usize, page: &str, url: String, taken: &AtomicU32) {
    let limit = shared.plan.per_source;
    if shared.job.cancelled() || shared.known.contains(&url) {
        return;
    }
    if taken.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| (n < limit).then_some(n + 1)).is_err() {
        return;
    }
    let kept = if !first_time(&shared.images_seen, &url) {
        Ok(false)
    } else if url.starts_with("data:") {
        save_inline(shared.root, shared.plan, page, &url).await
    } else {
        let request = SaveRequest {
            site: vault::site_of(&url),
            url,
            page_url: page.to_string(),
            kind: "image",
            stream: false,
            folder: shared.plan.folder.clone(),
            title: String::new(),
        };
        let saved = shared.downloads.save_now(shared.root.to_path_buf(), request, shared.plan.min_bytes, shared.job).await;
        saved.map(|path| path.is_some())
    };
    match kept {
        Ok(true) => status::update(&shared.sources, index, |source| source.saved += 1),
        Ok(false) => {
            taken.fetch_sub(1, Ordering::Relaxed);
        }
        Err(error) => {
            taken.fetch_sub(1, Ordering::Relaxed);
            tracing::warn!("A hunted image didn't save: {error}");
        }
    }
}

/// Reads one result page and saves its content images into source `index`.
async fn read_page(shared: &Shared<'_>, index: usize, page: String, taken: &AtomicU32) {
    let Ok(url) = Url::parse(&page) else { return };
    if !first_time(&shared.pages_seen, &page) {
        return;
    }
    let html = match fetch_page(&shared.client, &url).await {
        Ok(html) => html,
        Err(error) => {
            tracing::debug!("A result page was skipped: {error}");
            return;
        }
    };
    let kinds = shared.plan.kinds.clone();
    let found = tokio::task::spawn_blocking(move || content::images(&url, &html, &kinds)).await.unwrap_or_default();
    stream::iter(found).for_each_concurrent(AT_ONCE, |found| save_one(shared, index, &page, found.url, taken)).await;
}

/// Waits for `turn` (one per host), marks source `index` searching, runs `search`, and pauses before that host's next
/// search. `None` when the hunt was stopped first.
async fn in_turn<T>(shared: &Shared<'_>, index: usize, turn: usize, search: impl Future<Output = T>) -> Option<T> {
    let _turn = shared.turns[turn].lock().await;
    if shared.job.cancelled() {
        return None;
    }
    status::update(&shared.sources, index, |source| source.state = SourceState::Searching);
    let found = search.await;
    if !shared.job.cancelled() {
        tokio::time::sleep(shared.plan.pause).await;
    }
    Some(found)
}

/// Images for `query` from `engine`, up to twice the per-source limit.
async fn image_search(shared: &Shared<'_>, engine: ImageEngine, query: &str, requests: usize) -> Result<Vec<Hit>, String> {
    let stop = || shared.job.cancelled();
    let want = (shared.plan.per_source * SPARE) as usize;
    let ask = Ask { query, want, requests, pause: shared.plan.pause, stop: &stop };
    engines::search(&shared.client, &shared.plan.bases, engine, &ask).await
}

/// Whether `page` is on `site` or a subdomain of it.
fn on_site(page: &str, site: &str) -> bool {
    Url::parse(page).ok().and_then(|url| url.host_str().map(|host| host.trim_start_matches("www.").to_string())).is_some_and(|host| {
        host == site || host.ends_with(&format!(".{site}"))
    })
}

/// A site's image search results, then up to `pages` of the distinct pages they're on.
fn items_of(hits: Vec<Hit>, pages: usize) -> Vec<Item> {
    let mut read: Vec<String> = Vec::new();
    for hit in &hits {
        if read.len() < pages && hit.page != hit.image && !read.contains(&hit.page) {
            read.push(hit.page.clone());
        }
    }
    hits.into_iter().map(Item::Image).chain(read.into_iter().map(Item::Page)).collect()
}

/// A site's images and pages (ADR-0026): image search results on the site, from the first try that has any. Fails
/// only when no engine answered; `None` when the hunt was stopped.
async fn site_items(shared: &Shared<'_>, index: usize, site: &str) -> Option<Result<Vec<Item>, String>> {
    let topic = &shared.plan.query;
    let tries = [
        (ImageEngine::Bing, format!("site:{site} {topic}")),
        // Bing answers some `site:` searches (`site:wikipedia.org`) with nothing.
        (ImageEngine::Bing, format!("{topic} {site}")),
        (ImageEngine::DuckDuckGo, format!("site:{site} {topic}")),
    ];
    let mut answered = false;
    for (engine, query) in tries {
        if let Ok(hits) = in_turn(shared, index, engine.turn(), image_search(shared, engine, &query, SITE_REQUESTS)).await? {
            answered = true;
            let hits: Vec<Hit> = hits.into_iter().filter(|hit| on_site(&hit.page, site)).collect();
            if !hits.is_empty() {
                return Some(Ok(items_of(hits, shared.plan.pages as usize)));
            }
        }
    }
    Some(if answered { Ok(Vec::new()) } else { Err("Bing and DuckDuckGo didn't answer. Try again in a while.".into()) })
}

/// Goes through `items` of source `index`, `at_once` at a time, until the source's limit or Stop.
async fn work(shared: &Shared<'_>, index: usize, items: Vec<Item>, at_once: usize, taken: &AtomicU32) {
    stream::iter(items)
        .for_each_concurrent(at_once, |item| async move {
            if !shared.job.cancelled() && taken.load(Ordering::Relaxed) < shared.plan.per_source {
                match item {
                    Item::Page(page) => read_page(shared, index, page, taken).await,
                    Item::Image(hit) => save_one(shared, index, &hit.page, hit.image, taken).await,
                }
            }
            status::update(&shared.sources, index, |source| source.read += 1);
        })
        .await;
}

/// Hunts source `index`: searches when its host's turn comes, then saves its images and reads its pages a few at a
/// time, until its limit, the end of its results or Stop. Returns its problem when its search failed.
async fn hunt_source(shared: &Shared<'_>, index: usize, search: &Search) -> Option<String> {
    let Shared { plan, job, sources, .. } = shared;
    let stop = || job.cancelled();
    let found = match search {
        Search::Images(engine) => {
            let hits = image_search(shared, *engine, &plan.query, plan.pages as usize);
            in_turn(shared, index, engine.turn(), hits).await.map(|hits| hits.map(|hits| hits.into_iter().map(Item::Image).collect()))
        }
        Search::Pages(query) => {
            let pages = search::search(&shared.client, &plan.engine, query, plan.pages as usize, &stop);
            in_turn(shared, index, 0, pages).await.map(|pages| pages.map(|pages| pages.into_iter().map(Item::Page).collect()))
        }
        Search::Site(site) => site_items(shared, index, site).await,
    };
    let items = match found {
        None => {
            status::update(sources, index, |source| source.state = SourceState::Stopped);
            return None;
        }
        Some(Ok(items)) => items,
        Some(Err(problem)) => {
            status::update(sources, index, |source| {
                source.state = SourceState::Failed;
                source.problem = Some(problem.clone());
            });
            return Some(problem);
        }
    };
    status::update(sources, index, |source| {
        source.state = SourceState::Reading;
        source.found = u32::try_from(items.len()).unwrap_or(u32::MAX);
    });
    // Images first, many at once; then pages, a few at once.
    let (images, pages): (Vec<Item>, Vec<Item>) = items.into_iter().partition(|item| matches!(item, Item::Image(_)));
    let taken = AtomicU32::new(0);
    work(shared, index, images, IMAGES_AT_ONCE, &taken).await;
    work(shared, index, pages, PAGES_AT_ONCE, &taken).await;
    let state = if job.cancelled() { SourceState::Stopped } else { SourceState::Done };
    status::update(sources, index, |source| source.state = state);
    None
}

/// Moves the hunt folder's extra copies to the trash (ADR-0027) and returns how many moved. A failure is logged, not
/// the hunt's: its images stay.
async fn remove_duplicates(root: &Path, plan: &Plan, job: &Job, status: &Status) -> u32 {
    status::change(status, |hunt| hunt.removing = true);
    let (root, folder, scan) = (root.to_path_buf(), plan.folder.to_string_lossy().replace('\\', "/"), job.clone());
    let removed = match tokio::task::spawn_blocking(move || dedupe::remove_copies(&root, &folder, &scan)).await {
        Ok(Ok(removed)) => Some(removed),
        Ok(Err(error)) => {
            if !job.cancelled() {
                tracing::warn!("A hunt's duplicates weren't removed: {error}");
            }
            None
        }
        Err(error) => {
            tracing::warn!("A hunt's duplicate removal stopped: {error}");
            None
        }
    };
    status::change(status, |hunt| {
        hunt.removing = false;
        hunt.removed = removed;
    });
    removed.unwrap_or(0)
}

/// Runs every source, four at once, reporting images saved of the total as it goes. Fails only when nothing was saved
/// and a source failed.
async fn run(downloads: Downloads, root: PathBuf, plan: Plan, job: Job, sources: Status) -> Result<u64, String> {
    let client = net::client().timeout(PAGE_TIMEOUT).build().map_err(|e| format!("Can't start the hunt: {e}"))?;
    let shared = Shared {
        downloads: &downloads,
        root: &root,
        plan: &plan,
        job: &job,
        client,
        known: library::sources(&root)?,
        pages_seen: Mutex::default(),
        images_seen: Mutex::default(),
        turns: std::array::from_fn(|_| tokio::sync::Mutex::new(())),
        sources: sources.clone(),
    };
    let count = plan.searches.len();
    let total = u64::from(plan.per_source) * count as u64;
    let report = || {
        let (saved, done) = status::totals(&sources);
        job.items(saved, Some(total), Some(format!("{done} of {count} sources done")));
    };
    // Made up front: a `map` over the borrowed searches inside the stream trips the compiler's lifetime check for
    // spawned futures.
    let hunts: Vec<_> = plan.searches.iter().enumerate().map(|(index, (_, search))| hunt_source(&shared, index, search)).collect();
    let work = stream::iter(hunts).buffer_unordered(SOURCES_AT_ONCE).collect::<Vec<Option<String>>>();
    tokio::pin!(work);
    let problems = loop {
        tokio::select! {
            problems = &mut work => break problems,
            () = tokio::time::sleep(TICK) => report(),
        }
    };
    report();
    if job.cancelled() {
        return Err("Cancelled.".into());
    }
    let (saved, _) = status::totals(&sources);
    if saved == 0
        && let Some(problem) = problems.into_iter().flatten().next()
    {
        return Err(problem);
    }
    tracing::info!("A hunt saved {saved} images");
    let removed = if plan.remove_duplicates && saved > 1 { remove_duplicates(&root, &plan, &job, &sources).await } else { 0 };
    if job.cancelled() {
        return Err("Cancelled.".into());
    }
    Ok(saved - u64::from(removed))
}

// ── Public API ────────────────────────────
/// Starts a hunt into the library at `root` as a job, with the packs from `packs_dir`, with a row in the Download list
/// (ADR-0018). Progress reports images saved of the total; each source's own progress is in `hunts`. Must be called
/// inside a Tokio runtime.
pub fn hunt(
    downloads: &Downloads,
    jobs: &Arc<Jobs>,
    hunts: &Hunts,
    root: PathBuf,
    packs_dir: Option<&Path>,
    request: HuntRequest,
) -> Result<HuntStarted, String> {
    let title = request.query.trim().to_string();
    let plan = plan(packs_dir, request)?;
    let folder = plan.folder.to_string_lossy().replace('\\', "/");
    let row = Row { source: DownloadSource::Hunt, url: String::new(), title, kind: "Images".into(), thumbnail: None };
    let sources = waiting(&plan);
    let saver = downloads.clone();
    let kept = sources.clone();
    let id = downloads.track(jobs, row, folder.clone(), move |job| run(saver, root, plan, job, kept));
    hunts.begin(id, sources);
    Ok(HuntStarted { id, folder })
}

#[cfg(test)]
mod tests {
    use super::{HuntRequest, ImageEngine, ImageKind, Search, SourceState, decode_data, on_site, plan, run, waiting};
    use crate::crawl::test_site::serve;
    use crate::hunt::HuntSource;
    use crate::jobs::{JobEvent, JobResult, Jobs};
    use crate::media::Downloads;
    use crate::tools::Toolbox;
    use crate::vault::{KeySource, Vault};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    fn request(pack: &str, web: bool, sites: &[&str]) -> HuntRequest {
        HuntRequest {
            query: "cats".into(),
            pack: pack.into(),
            web,
            sites: sites.iter().map(|site| site.to_string()).collect(),
            engines: Vec::new(),
            kinds: vec![ImageKind::Img, ImageKind::Inline],
            pages: 10,
            per_source: 50,
            min_kb: 1,
            remove_duplicates: false,
        }
    }

    fn names(request: HuntRequest, packs_dir: Option<&std::path::Path>) -> Vec<String> {
        plan(packs_dir, request).unwrap().searches.into_iter().map(|(name, _)| name).collect()
    }

    #[test]
    fn checks_the_request() {
        let games = plan(None, request("games", true, &["fandom.com", "evil.com"])).unwrap();
        assert!(matches!(&games.searches[..], [(_, Search::Site(site))] if site == "fandom.com"), "a pack without the web search ignores `web`");
        assert_eq!(games.folder, std::path::PathBuf::from("Images/cats"));

        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("mine.toml"), "name = \"Mine\"\nweb = true\nsites = [\"a.com\"]\n").unwrap();
        let engines = vec![ImageEngine::Yandex, ImageEngine::DuckDuckGo, ImageEngine::Yandex];
        let all = names(HuntRequest { engines, ..request("mine", true, &["a.com"]) }, Some(dir.path()));
        assert_eq!(all, ["DuckDuckGo Images", "Yandex Images", "Any site", "a.com"], "engines first, each once");
        assert_eq!(plan(None, HuntRequest { per_source: 0, ..request("games", false, &["fandom.com"]) }).unwrap().per_source, 1);

        // Engines take whole images: they need no image kinds.
        let only_engines = HuntRequest { engines: vec![ImageEngine::Bing], kinds: Vec::new(), ..request("games", false, &[]) };
        assert_eq!(names(only_engines, None), ["Bing Images"]);
        assert!(plan(None, HuntRequest { kinds: Vec::new(), ..request("games", false, &["fandom.com"]) }).is_err());
        assert!(plan(None, request("gone", true, &[])).is_err());
        assert!(plan(None, request("general", false, &[])).unwrap_err().contains("place"));
        let (bytes, ext) = decode_data("data:image/svg+xml,%3Csvg%3E%3C%2Fsvg%3E").unwrap();
        assert_eq!((bytes.as_slice(), ext), (b"<svg></svg>".as_slice(), "svg"));
        assert!(decode_data("data:text/html;base64,PGI+").is_none());
        assert!(on_site("https://en.wikipedia.org/wiki/Cat", "wikipedia.org") && on_site("https://www.reddit.com/r/cats", "reddit.com"));
        assert!(!on_site("https://notreddit.com/", "reddit.com") && !on_site("not a link", "reddit.com"));
    }

    /// A web search listing an article twice; the article (a big and a tiny picture, an inline one, and a menu); and
    /// Bing's first page of image results for "cats" (a picture) and for `site:127.0.0.1 cats` (another, on the
    /// article). Anything else (`nowhere.test`'s searches, later pages) isn't there.
    fn route(path: &str, address: &str) -> Option<(&'static str, Vec<u8>)> {
        let inline = format!("data:image/png;base64,{}", base64::Engine::encode(&base64::engine::general_purpose::STANDARD, [9u8; 2048]));
        let bing = |image: &str| {
            let m = format!(r#"{{"purl":"{address}article","murl":"{address}{image}"}}"#).replace('"', "&quot;");
            ("text/html", format!(r#"<a class="iusc" m="{m}"></a>"#).into_bytes())
        };
        Some(match path {
            "/html/?q=cats" => {
                let body = format!(r#"<a class="result__a" href="{address}article">A</a><a class="result__a" href="{address}article">B</a>"#);
                ("text/html", body.into_bytes())
            }
            "/article" => {
                let body = format!(
                    r#"<body><nav><img src="/menu.png"></nav><article><p>Cats sleep for most of the day, and they like warm places, boxes and sun.</p>
                    <img src="/big.png"><img src="/tiny.png"><img src="{inline}"><img src="/big.png"></article></body>"#
                );
                ("text/html", body.into_bytes())
            }
            "/images/async?q=cats&first=0&count=35&mmasync=1" => bing("photo.png"),
            "/images/async?q=site%3A127.0.0.1+cats&first=0&count=35&mmasync=1" => bing("photo2.png"),
            "/big.png" | "/menu.png" | "/photo.png" | "/photo2.png" => ("image/png", vec![7u8; 4096]),
            "/tiny.png" => ("image/png", vec![7u8; 100]),
            _ => return None,
        })
    }

    /// Hunts four sources (any site, a site Bing finds, a site no engine answers for, and Bing) with `per_source`;
    /// returns the files saved, each source as it ended, and how the job ended.
    async fn hunt_four(per_source: u32) -> (Vec<String>, Vec<HuntSource>, JobResult) {
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
        let mut checked = plan(None, HuntRequest { per_source, ..request("games", false, &["fandom.com"]) }).unwrap();
        checked.searches = vec![
            ("Any site".into(), Search::Pages("cats".into())),
            ("127.0.0.1".into(), Search::Site("127.0.0.1".into())),
            ("nowhere.test".into(), Search::Site("nowhere.test".into())),
            ("Bing Images".into(), Search::Images(ImageEngine::Bing)),
        ];
        let base = address.trim_end_matches('/').to_string();
        checked.engine = format!("{address}html/");
        checked.bases.bing = base.clone();
        checked.bases.duckduckgo = base;
        checked.pause = Duration::ZERO;
        let folder = root.join(&checked.folder);
        let sources = waiting(&checked);
        let kept = sources.clone();

        jobs.start(move |job| async move { run(downloads, root, checked, job, kept).await.map(|_| ()) });
        while ended.lock().unwrap().is_empty() {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        let mut names: Vec<String> =
            std::fs::read_dir(folder).unwrap().map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned()).collect();
        names.sort();
        let result = ended.lock().unwrap()[0].clone();
        let sources = sources.lock().unwrap().sources.clone();
        (names, sources, result)
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn hunts_each_source_into_the_topics_folder() {
        let (names, sources, result) = hunt_four(50).await;
        assert_eq!(result, JobResult::Done, "a failed source doesn't fail a hunt that saved images");
        assert_eq!(names.len(), 4, "{names:?}");
        assert!(names[0] == "big.png" && names[1].starts_with("inline-") && names[2] == "photo.png" && names[3] == "photo2.png");
        let states: Vec<SourceState> = sources.iter().map(|source| source.state).collect();
        assert_eq!(states, [SourceState::Done, SourceState::Done, SourceState::Failed, SourceState::Done]);
        // The site's image search result and its page: two results.
        assert_eq!((sources[1].found, sources[1].read), (2, 2));
        assert!(sources[1].saved >= 1);
        assert_eq!(sources[2].problem.as_deref(), Some("Bing and DuckDuckGo didn't answer. Try again in a while."));
        assert_eq!(sources.iter().map(|source| source.saved).sum::<u32>(), 4);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn keeps_each_source_to_its_limit() {
        let (names, sources, _) = hunt_four(1).await;
        assert_eq!(names.len(), 3, "one each from any site, the site and Bing: {names:?}");
        assert!(sources.iter().all(|source| source.saved <= 1 && source.limit == 1));
    }

    /// Proves site sources work on real sites: run with `cargo test -p scrappy-core -- --ignored`.
    #[tokio::test(flavor = "multi_thread")]
    #[ignore = "searches Bing and DuckDuckGo and saves a few images from reddit, Wikipedia and Flickr"]
    async fn hunts_real_sites() {
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
        let sites = ["reddit.com", "wikipedia.org", "flickr.com"];
        let request = HuntRequest { query: "red panda".into(), per_source: 3, pages: 3, ..request("general", false, &sites) };
        let checked = plan(None, request).unwrap();
        let sources = waiting(&checked);
        let kept = sources.clone();
        jobs.start(move |job| async move { run(downloads, root, checked, job, kept).await.map(|_| ()) });
        while ended.lock().unwrap().is_empty() {
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
        for source in sources.lock().unwrap().sources.iter() {
            eprintln!("{}: {:?}, {} of {} results, {} saved {:?}", source.name, source.state, source.read, source.found, source.saved, source.problem);
            assert!(source.saved > 0, "{} saved nothing", source.name);
        }
    }
}

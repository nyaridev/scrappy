//! Run: a hunt as one job. Each search (the whole web, then each picked site of the pack) gives result pages; each
//! page is read as plain HTTP, the content finder lists its images, and they're saved into `Images/<topic>/` (web
//! images through `media`, inline ones written here), skipping links the catalog already has (ADR-0013).

// ── Imports ───────────────────────────────
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use futures_util::{StreamExt, stream};
use reqwest::header::CONTENT_TYPE;
use reqwest::{Client, Url};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use ts_rs::TS;

use super::content::{self, Found, ImageKind};
use super::{pack, search};
use crate::app::USER_AGENT;
use crate::jobs::{Job, JobId, Jobs};
use crate::library::{self, Record, template};
use crate::media::{Downloads, SaveRequest};
use crate::vault;

// ── Constants ─────────────────────────────
const IMAGES: &str = "Images";
/// Images saved at once.
const AT_ONCE: usize = 4;
const MAX_QUERY: usize = 200;
const MAX_PAGES: u32 = 50;
const MAX_FILES: u32 = 5000;
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
    /// Search the whole web (when the pack does).
    pub web: bool,
    /// The pack's sites to search.
    pub sites: Vec<String>,
    pub kinds: Vec<ImageKind>,
    /// The most result pages opened per search.
    pub pages: u32,
    /// The most images saved.
    pub files: u32,
    /// Images under this many kilobytes aren't kept: icons, thumbnails.
    pub min_kb: u32,
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

/// A request that passed its checks.
#[derive(Debug)]
struct Plan {
    /// Each search: what the progress calls it, and what's searched.
    searches: Vec<(String, String)>,
    /// The search page's address (a local one in tests).
    engine: String,
    kinds: Vec<ImageKind>,
    folder: PathBuf,
    pages: u32,
    files: u32,
    min_bytes: u64,
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
    if request.web && pack.web {
        searches.push(("the web".to_string(), query.clone()));
    }
    for site in pack.sites.iter().filter(|site| request.sites.contains(site)) {
        searches.push((site.clone(), format!("site:{site} {query}")));
    }
    if searches.is_empty() {
        return Err("Pick at least one place to search.".into());
    }
    if request.kinds.is_empty() {
        return Err("Pick at least one kind of image.".into());
    }
    Ok(Plan {
        searches,
        engine: search::ENGINE.to_string(),
        kinds: request.kinds,
        folder: PathBuf::from(IMAGES).join(template::sanitize(&query)),
        pages: request.pages.clamp(1, MAX_PAGES),
        files: request.files.clamp(1, MAX_FILES),
        min_bytes: u64::from(request.min_kb) * 1024,
    })
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

/// Saves one page's images a few at a time; returns how many were kept.
async fn save(downloads: &Downloads, root: &Path, plan: &Plan, page: &str, found: Vec<Found>, job: &Job, report: impl Fn(u64)) -> u64 {
    let mut saves = stream::iter(found)
        .map(|found| async move {
            if job.cancelled() {
                return Ok(false);
            }
            if found.url.starts_with("data:") {
                return save_inline(root, plan, page, &found.url).await;
            }
            let request = SaveRequest {
                site: vault::site_of(&found.url),
                url: found.url,
                page_url: page.to_string(),
                kind: "image",
                stream: false,
                folder: plan.folder.clone(),
                title: String::new(),
            };
            downloads.save_now(root.to_path_buf(), request, plan.min_bytes, job).await.map(|path| path.is_some())
        })
        .buffer_unordered(AT_ONCE);
    let mut kept = 0;
    while let Some(result) = saves.next().await {
        match result {
            Ok(true) => kept += 1,
            Ok(false) => {}
            Err(error) => tracing::warn!("A hunted image didn't save: {error}"),
        }
        report(kept);
    }
    kept
}

/// Runs each search, reads its pages and saves their content images, until the image limit, the end of the results
/// or the job is stopped.
async fn run(downloads: Downloads, root: PathBuf, plan: Plan, job: Job) -> Result<(), String> {
    let client = Client::builder().user_agent(USER_AGENT).timeout(PAGE_TIMEOUT).build().map_err(|e| format!("Can't start the hunt: {e}"))?;
    let known = library::sources(&root)?;
    let (mut pages_seen, mut images_seen) = (HashSet::new(), HashSet::new());
    let (mut saved, mut problems) = (0u64, Vec::new());
    let limit = u64::from(plan.files);
    let stop = || job.cancelled();
    for (label, query) in &plan.searches {
        if job.cancelled() || saved >= limit {
            break;
        }
        job.items(saved, Some(limit), Some(format!("Searching {label}")));
        let pages = match search::search(&client, &plan.engine, query, plan.pages as usize, &stop).await {
            Ok(pages) => pages,
            Err(error) => {
                problems.push(error);
                continue;
            }
        };
        for page in pages {
            if job.cancelled() || saved >= limit {
                break;
            }
            let Ok(url) = Url::parse(&page) else { continue };
            if !pages_seen.insert(page.clone()) {
                continue;
            }
            let host = url.host_str().unwrap_or_default().trim_start_matches("www.").to_string();
            job.items(saved, Some(limit), Some(host.clone()));
            let html = match fetch_page(&client, &url).await {
                Ok(html) => html,
                Err(error) => {
                    tracing::debug!("A result page was skipped: {error}");
                    continue;
                }
            };
            let kinds = plan.kinds.clone();
            let found = tokio::task::spawn_blocking(move || content::images(&url, &html, &kinds)).await.unwrap_or_default();
            let room = usize::try_from(limit - saved).unwrap_or(usize::MAX);
            let fresh: Vec<Found> = found
                .into_iter()
                .filter(|found| !known.contains(&found.url) && images_seen.insert(found.url.clone()))
                .take(room)
                .collect();
            let before = saved;
            let report = |kept| job.items(before + kept, Some(limit), Some(host.clone()));
            saved += save(&downloads, &root, &plan, &page, fresh, &job, report).await;
        }
    }
    if job.cancelled() {
        return Err("Cancelled.".into());
    }
    if saved == 0
        && let Some(problem) = problems.into_iter().next()
    {
        return Err(problem);
    }
    tracing::info!("A hunt saved {saved} images");
    Ok(())
}

// ── Public API ────────────────────────────
/// Starts a hunt into the library at `root` as a job, with the packs from `packs_dir`. Progress reports images saved
/// of the limit, with the search or the site being read as its message. Must be called inside a Tokio runtime.
pub fn hunt(downloads: &Downloads, jobs: &Arc<Jobs>, root: PathBuf, packs_dir: Option<&Path>, request: HuntRequest) -> Result<HuntStarted, String> {
    let plan = plan(packs_dir, request)?;
    let folder = plan.folder.to_string_lossy().replace('\\', "/");
    let downloads = downloads.clone();
    let id = jobs.start(move |job| run(downloads, root, plan, job));
    Ok(HuntStarted { id, folder })
}

#[cfg(test)]
mod tests {
    use super::{HuntRequest, ImageKind, decode_data, plan, run};
    use crate::crawl::test_site::serve;
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
            kinds: vec![ImageKind::Img, ImageKind::Inline],
            pages: 10,
            files: 50,
            min_kb: 1,
        }
    }

    #[test]
    fn checks_the_request() {
        let games = plan(None, request("games", true, &["fandom.com", "evil.com"])).unwrap();
        let searches: Vec<&str> = games.searches.iter().map(|(_, query)| query.as_str()).collect();
        assert_eq!(searches, ["cats", "site:fandom.com cats"]);
        assert_eq!(games.folder, std::path::PathBuf::from("Images/cats"));
        assert!(plan(None, request("gone", true, &[])).is_err());
        assert!(plan(None, request("general", false, &[])).unwrap_err().contains("place"));
        assert!(plan(None, HuntRequest { kinds: Vec::new(), ..request("general", true, &[]) }).is_err());
        let (bytes, ext) = decode_data("data:image/svg+xml,%3Csvg%3E%3C%2Fsvg%3E").unwrap();
        assert_eq!((bytes.as_slice(), ext), (b"<svg></svg>".as_slice(), "svg"));
        assert!(decode_data("data:text/html;base64,PGI+").is_none());
    }

    /// A search page listing an article twice, and the article: a big and a tiny picture, an inline one, and a menu.
    fn route(path: &str, address: &str) -> Option<(&'static str, Vec<u8>)> {
        let inline = format!("data:image/png;base64,{}", base64::Engine::encode(&base64::engine::general_purpose::STANDARD, [9u8; 2048]));
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
            "/big.png" | "/menu.png" => ("image/png", vec![7u8; 4096]),
            "/tiny.png" => ("image/png", vec![7u8; 100]),
            _ => return None,
        })
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn hunts_content_images_into_the_topics_folder() {
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
        let mut checked = plan(None, request("general", true, &[])).unwrap();
        checked.engine = format!("{address}html/");
        let folder = root.join(&checked.folder);

        jobs.start(move |job| run(downloads, root, checked, job));
        while ended.lock().unwrap().is_empty() {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        assert_eq!(ended.lock().unwrap()[0], JobResult::Done);
        let mut names: Vec<String> =
            std::fs::read_dir(folder).unwrap().map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned()).collect();
        names.sort();
        assert_eq!(names.len(), 2, "{names:?}");
        assert!(names[0] == "big.png" && names[1].starts_with("inline-") && names[1].ends_with(".png"));
    }
}

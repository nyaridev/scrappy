//! Request: what a crawl asks for (a site's files, a mirror of it, or one page as one file), its checks, and starting it
//! as one job (ADR-0008).

// ── Imports ───────────────────────────────
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use reqwest::Url;
use serde::{Deserialize, Serialize};
use spider::website::Website;
use ts_rs::TS;

use super::{files, mirror, page};
use crate::app::USER_AGENT;
use crate::grab::FindKind;
use crate::jobs::{JobId, Jobs};
use crate::library::template;
use crate::media::Downloads;
use crate::vault;

// ── Constants ─────────────────────────────
const FILES_FOLDER: &str = "Images/{site}";
const SITES_FOLDER: &str = "Sites/{site}";
const MAX_DEPTH: u32 = 10;
const MAX_PAGES: u32 = 10_000;
const MAX_FILES: u32 = 10_000;
const MAX_DELAY_MS: u32 = 60_000;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

// ── Types ─────────────────────────────────
/// What a crawl keeps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum CrawlMode {
    /// The site's files of the wanted kinds, into `Images/<domain>/`.
    Files,
    /// The site's pages with what they need to show offline, into `Sites/<domain>/`.
    Mirror,
    /// The one page, everything inlined, into `Sites/<domain>/_pages/`.
    Page,
}

/// What to crawl and its limits. A page save uses only the link; kinds and the smallest size are for files.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CrawlRequest {
    pub url: String,
    pub mode: CrawlMode,
    /// How many links away from the first page it goes.
    pub depth: u32,
    /// The most pages it visits.
    pub pages: u32,
    /// The most files it saves.
    pub files: u32,
    /// Milliseconds between two requests to the site.
    pub delay_ms: u32,
    /// The kinds of files kept; streams aren't (they need Grab).
    pub kinds: Vec<FindKind>,
    /// Files under this many kilobytes aren't kept: icons, spacers.
    pub min_kb: u32,
}

/// A crawl that started: its job, the folder it saves to, and the page to open once it's done.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CrawlStarted {
    pub id: JobId,
    /// Relative to the library root.
    pub folder: String,
    /// A mirror's start page or the saved page, relative to the library root.
    pub open: Option<String>,
}

/// A request that passed its checks.
#[derive(Debug)]
pub(super) struct Plan {
    pub url: Url,
    pub mode: CrawlMode,
    pub site: String,
    /// Relative to the library root.
    pub folder: PathBuf,
    pub depth: u32,
    pub pages: u32,
    pub files: u32,
    pub delay_ms: u32,
    pub kinds: Vec<FindKind>,
    pub min_bytes: u64,
}

// ── Private helpers ───────────────────────
fn slash(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// Checks the link and keeps every limit in range.
pub(super) fn plan(request: CrawlRequest) -> Result<Plan, String> {
    let url = Url::parse(request.url.trim()).map_err(|_| "Not a link.".to_string())?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err("Only http and https links work.".into());
    }
    let site = vault::site_of(url.as_str()).ok_or("This link has no site.")?;
    let kinds: Vec<FindKind> = request.kinds.into_iter().filter(|kind| *kind != FindKind::Stream).collect();
    if request.mode == CrawlMode::Files && kinds.is_empty() {
        return Err("Pick at least one kind of file.".into());
    }
    let fields = template::Fields { site: Some(site.clone()), ..template::Fields::default() };
    let folder = if request.mode == CrawlMode::Files { FILES_FOLDER } else { SITES_FOLDER };
    Ok(Plan {
        url,
        mode: request.mode,
        folder: template::render(folder, &fields),
        site,
        depth: request.depth.clamp(1, MAX_DEPTH),
        pages: request.pages.clamp(1, MAX_PAGES),
        files: request.files.clamp(1, MAX_FILES),
        delay_ms: request.delay_ms.min(MAX_DELAY_MS),
        kinds,
        min_bytes: u64::from(request.min_kb) * 1024,
    })
}

/// The crawler for `plan`: same site only, within its depth, pages and delay, respecting `robots.txt`.
pub(super) fn website(plan: &Plan) -> Website {
    let mut website = Website::new(plan.url.as_str());
    website
        .with_depth(plan.depth as usize)
        .with_limit(plan.pages)
        .with_delay(u64::from(plan.delay_ms))
        .with_respect_robots_txt(true)
        .with_subdomains(false)
        .with_user_agent(Some(USER_AGENT))
        .with_request_timeout(Some(REQUEST_TIMEOUT));
    website
}

/// "1 page", "3 pages".
pub(super) fn pages(count: usize) -> String {
    if count == 1 { "1 page".into() } else { format!("{count} pages") }
}

// ── Public API ────────────────────────────
/// Starts a crawl of `request.url` into the library at `root` as a job. A file crawl or a mirror reports files saved
/// of the limit, with the pages visited as its message. Must be called inside a Tokio runtime.
pub fn crawl(downloads: &Downloads, jobs: &Arc<Jobs>, root: PathBuf, request: CrawlRequest) -> Result<CrawlStarted, String> {
    let plan = plan(request)?;
    let folder = slash(&plan.folder);
    let open = match plan.mode {
        CrawlMode::Files => None,
        CrawlMode::Mirror => Some(mirror::start_page(&plan.url)),
        CrawlMode::Page => Some(page::file(&plan.url)),
    }
    .map(|file| slash(&plan.folder.join(file)));
    let id = match plan.mode {
        CrawlMode::Files => {
            let downloads = downloads.clone();
            jobs.start(move |job| files::run(downloads, root, plan, job))
        }
        CrawlMode::Mirror => jobs.start(move |job| mirror::run(root, plan, job)),
        CrawlMode::Page => jobs.start(move |job| page::run(root, plan, job)),
    };
    Ok(CrawlStarted { id, folder, open })
}

#[cfg(test)]
mod tests {
    use super::{CrawlMode, CrawlRequest, plan};
    use crate::crawl::test_site::request;
    use crate::grab::FindKind;
    use std::path::PathBuf;

    #[test]
    fn checks_the_request() {
        assert!(plan(request("file:///C:/", CrawlMode::Files)).is_err());
        let streams = CrawlRequest { kinds: vec![FindKind::Stream], ..request("https://a.com/", CrawlMode::Files) };
        assert!(plan(streams).unwrap_err().contains("Pick"));
        let mirror = CrawlRequest { kinds: Vec::new(), ..request("https://a.com/", CrawlMode::Mirror) };
        assert_eq!(plan(mirror).unwrap().folder, PathBuf::from("Sites/a.com"));
        let huge = CrawlRequest { depth: 99, pages: 0, ..request("https://www.a.com/x", CrawlMode::Files) };
        let checked = plan(huge).unwrap();
        assert_eq!((checked.depth, checked.pages, checked.site.as_str()), (10, 1, "a.com"));
        assert_eq!(checked.folder, PathBuf::from("Images/a.com"));
    }
}

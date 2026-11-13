//! Request: what a crawl asks for (a mirror of a site, or one page as one file), its checks, and starting it as one job
//! (ADR-0008). A site's files are Grab's (ADR-0031).

// ── Imports ───────────────────────────────
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use reqwest::Url;
use serde::{Deserialize, Serialize};
use spider::website::Website;
use ts_rs::TS;

use super::mirrors::Mirrors;
use super::{mirror, page};
use crate::app::USER_AGENT;
use crate::jobs::{JobId, Jobs};
use crate::library::template;
use crate::net;
use crate::media::{DownloadSource, Downloads, Row};
use crate::vault;

// ── Constants ─────────────────────────────
const SITES_FOLDER: &str = "Sites/{site}";
const MAX_DEPTH: u32 = 10;
const MAX_PAGES: u32 = 10_000;
const MAX_DELAY_MS: u32 = 60_000;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

// ── Types ─────────────────────────────────
/// What a crawl keeps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum CrawlMode {
    /// The site's pages with what they need to show offline, into `Sites/<domain>/`.
    Mirror,
    /// The one page, everything inlined, into `Sites/<domain>/_pages/`.
    Page,
}

/// What to crawl and its limits. A page save uses only the link.
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
    /// Milliseconds between two requests to the site.
    pub delay_ms: u32,
}

/// A crawl that started: its job, its site (a mirror's media are asked for by it), the folder it saves to, and the page
/// to open once it's done.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CrawlStarted {
    pub id: JobId,
    pub site: String,
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
    pub delay_ms: u32,
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
    let fields = template::Fields { site: Some(site.clone()), ..template::Fields::default() };
    Ok(Plan {
        url,
        mode: request.mode,
        folder: template::render(SITES_FOLDER, &fields),
        site,
        depth: request.depth.clamp(1, MAX_DEPTH),
        pages: request.pages.clamp(1, MAX_PAGES),
        delay_ms: request.delay_ms.min(MAX_DELAY_MS),
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
        .with_proxies(net::proxy().map(|proxy| vec![proxy]))
        .with_request_timeout(Some(REQUEST_TIMEOUT));
    website
}

/// "1 page", "3 pages".
pub(super) fn pages(count: usize) -> String {
    if count == 1 { "1 page".into() } else { format!("{count} pages") }
}

// ── Public API ────────────────────────────
/// Starts a crawl of `request.url` into the library at `root` as a job, with a row in the Download list (ADR-0018). A
/// mirror reports the files saved, with the pages visited as its message, and lists its media in `mirrors`
/// (ADR-0033). Must be called inside a Tokio runtime.
pub fn crawl(
    downloads: &Downloads,
    jobs: &Arc<Jobs>,
    mirrors: &Mirrors,
    root: PathBuf,
    request: CrawlRequest,
) -> Result<CrawlStarted, String> {
    let plan = plan(request)?;
    let folder = slash(&plan.folder);
    let open = match plan.mode {
        CrawlMode::Mirror => mirror::start_page(&plan.url),
        CrawlMode::Page => page::file(&plan.url),
    };
    let open = Some(slash(&plan.folder.join(open)));
    let kind = match plan.mode {
        CrawlMode::Mirror => "Mirror",
        CrawlMode::Page => "Page",
    };
    let (url, site) = (plan.url.to_string(), plan.site.clone());
    let row = Row { source: DownloadSource::Scrape, url, title: site.clone(), kind: kind.into(), thumbnail: None };
    let id = match plan.mode {
        CrawlMode::Mirror => {
            let mirrors = mirrors.clone();
            downloads.track(jobs, row, folder.clone(), move |job| mirror::run(root, plan, job, mirrors))
        }
        CrawlMode::Page => downloads.track(jobs, row, folder.clone(), move |job| page::run(root, plan, job)),
    };
    Ok(CrawlStarted { id, site, folder, open })
}

#[cfg(test)]
mod tests {
    use super::{CrawlMode, CrawlRequest, plan};
    use crate::crawl::test_site::request;
    use std::path::PathBuf;

    #[test]
    fn checks_the_request() {
        assert!(plan(request("file:///C:/", CrawlMode::Mirror)).is_err());
        assert_eq!(plan(request("https://a.com/", CrawlMode::Mirror)).unwrap().folder, PathBuf::from("Sites/a.com"));
        let huge = CrawlRequest { depth: 99, pages: 0, ..request("https://www.a.com/x", CrawlMode::Page) };
        let checked = plan(huge).unwrap();
        assert_eq!((checked.depth, checked.pages, checked.site.as_str()), (10, 1, "a.com"));
    }
}

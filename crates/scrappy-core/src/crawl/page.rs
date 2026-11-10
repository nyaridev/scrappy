//! Page: one page saved as one file with `monolith`, its stylesheets, scripts, images and fonts inlined, into
//! `Sites/<domain>/_pages/` (ADR-0008).

// ── Imports ───────────────────────────────
use std::path::PathBuf;

use monolith::core::{MonolithOptions, create_monolithic_document};
use monolith::session::Session;
use reqwest::Url;

use super::local::local_path;
use super::request::Plan;
use crate::app::USER_AGENT;
use crate::jobs::Job;
use crate::library::{self, Record};

// ── Constants ─────────────────────────────
const PAGES: &str = "_pages";
const TIMEOUT_SECS: u64 = 30;
const MAX_NAME: usize = 150;

// ── Public API ────────────────────────────
/// Where a saved page goes in its site's folder: `https://a.com/blog/post` → `_pages/blog-post.html`.
pub(super) fn file(url: &Url) -> PathBuf {
    let local = local_path(url, url.host_str().unwrap_or_default(), true).to_string_lossy().replace('\\', "/");
    let name = local.strip_suffix("index.html").unwrap_or(&local).trim_end_matches('/');
    let name = name.strip_suffix(".html").or_else(|| name.strip_suffix(".htm")).unwrap_or(name);
    let name: String = if name.is_empty() { "index".into() } else { name.replace('/', "-").chars().take(MAX_NAME).collect() };
    PathBuf::from(PAGES).join(format!("{name}.html"))
}

/// Saves the page as one file and adds it to the catalog as a `site` row.
pub(super) async fn run(root: PathBuf, plan: Plan, job: Job) -> Result<(), String> {
    job.items(0, Some(1), None);
    let url = plan.url.to_string();
    let saved = tokio::task::spawn_blocking(move || {
        let options = MonolithOptions {
            user_agent: Some(USER_AGENT.into()),
            timeout: TIMEOUT_SECS,
            silent: true,
            ignore_errors: true,
            ..MonolithOptions::default()
        };
        create_monolithic_document(Session::new(None, None, options), url)
    })
    .await
    .map_err(|_| "The page save stopped.".to_string())?;
    let (html, _title) = saved.map_err(|e| format!("Can't save the page: {e}"))?;
    if job.cancelled() {
        return Err("Cancelled.".into());
    }
    let path = plan.folder.join(file(&plan.url));
    let target = root.join(&path);
    if let Some(parent) = target.parent() {
        tokio::fs::create_dir_all(parent).await.map_err(|e| format!("Can't make the page's folder: {e}"))?;
    }
    tokio::fs::write(&target, &html).await.map_err(|e| format!("Can't save the page: {e}"))?;
    library::record(
        &root,
        &Record {
            path: path.to_string_lossy().replace('\\', "/"),
            kind: "site",
            source_url: plan.url.to_string(),
            site: Some(plan.site.clone()),
            user: None,
            size_bytes: html.len() as u64,
            duration_secs: None,
            codec: None,
            transferred_bytes: html.len() as u64,
        },
    )?;
    job.items(1, Some(1), None);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::file;
    use crate::crawl::test_site::{request, serve};
    use crate::crawl::{CrawlMode, crawl};
    use crate::jobs::{JobEvent, JobResult, Jobs};
    use crate::media::Downloads;
    use crate::tools::Toolbox;
    use crate::vault::{KeySource, Vault};
    use reqwest::Url;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    #[test]
    fn names_the_saved_page() {
        let name = |url: &str| file(&Url::parse(url).unwrap()).to_string_lossy().replace('\\', "/");
        assert_eq!(name("https://a.com/"), "_pages/index.html");
        assert_eq!(name("https://a.com/blog/post"), "_pages/blog-post.html");
        assert_eq!(name("https://a.com/a/page.html"), "_pages/a-page.html");
    }

    fn route(path: &str, _: &str) -> Option<(&'static str, Vec<u8>)> {
        Some(match path {
            "/" => ("text/html", br#"<html><head><link rel="stylesheet" href="/s.css"></head><body><img src="/a.png"></body></html>"#.to_vec()),
            "/s.css" => ("text/css", b"p { color: red }".to_vec()),
            "/a.png" => ("image/png", vec![7u8; 64]),
            _ => return None,
        })
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn saves_a_page_as_one_file() {
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

        let started = crawl(&downloads, &jobs, root.clone(), request(&address, CrawlMode::Page)).unwrap();
        while ended.lock().unwrap().is_empty() {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        assert_eq!(ended.lock().unwrap()[0], JobResult::Done);
        let page = std::fs::read_to_string(root.join(started.open.unwrap())).unwrap();
        assert!(page.contains("color: red") && page.contains("data:image/png;base64,"), "{page}");
    }
}

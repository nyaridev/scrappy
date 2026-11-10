//! Scan: Grab's deep scan. Opens the page in the installed Chrome, Brave or Edge, headless and with a fresh profile
//! (never the user's: no cookies, no history), records every response for a while, plays the page's videos so their
//! streams load, then adds the page's links to files (ADR-0007).

// ── Imports ───────────────────────────────
use std::path::Path;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use chromiumoxide::browser::{Browser, BrowserConfig};
use chromiumoxide::cdp::browser_protocol::network::{EnableParams, EventResponseReceived};
use futures_util::StreamExt;
use serde::Serialize;
use serde_json::Value;
use ts_rs::TS;

use super::find::{Find, Seen, collect};
use crate::tools;

// ── Constants ─────────────────────────────
const LAUNCH_TIMEOUT: Duration = Duration::from_secs(30);
const LOAD_TIMEOUT: Duration = Duration::from_secs(30);
/// How long the page runs after it loads, so players start and lazy images arrive.
const SETTLE: Duration = Duration::from_secs(8);
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

// ── Types ─────────────────────────────────
/// What a scan found on a page.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PageScan {
    /// The page's address once it loaded (after redirects).
    pub url: String,
    pub title: Option<String>,
    pub finds: Vec<Find>,
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

// ── Public API ────────────────────────────
/// Scans the page at `url` with the installed Chrome, Brave or Edge and returns what it found.
pub async fn scan(url: &str) -> Result<PageScan, String> {
    let url = reqwest::Url::parse(url.trim()).map_err(|_| "Not a link.".to_string())?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err("Only http and https links work.".into());
    }
    let program = tools::browser().ok_or("Scanning needs Chrome, Brave or Edge installed.")?;
    scan_with(&program, url.as_str()).await
}

/// Scans `url` (already checked) with the browser at `program`.
async fn scan_with(program: &Path, url: &str) -> Result<PageScan, String> {
    let profile = tempfile::Builder::new().prefix("scrappy-scan-").tempdir().map_err(|e| format!("Can't scan: {e}"))?;
    let config = BrowserConfig::builder()
        .chrome_executable(program)
        .user_data_dir(profile.path())
        .new_headless_mode()
        .launch_timeout(LAUNCH_TIMEOUT)
        .args(["--mute-audio", "--no-first-run", "--no-default-browser-check", "--autoplay-policy=no-user-gesture-required"])
        .build()?;
    let (mut browser, mut handler) = Browser::launch(config).await.map_err(failed("start"))?;
    let events = tokio::spawn(async move { while handler.next().await.is_some() {} });
    let result = run(&browser, url).await;
    let _ = browser.close().await;
    let _ = browser.wait().await;
    events.abort();
    result
}

/// Loads the page in a new tab and collects its responses and links.
async fn run(browser: &Browser, url: &str) -> Result<PageScan, String> {
    let page = browser.new_page("about:blank").await.map_err(failed("open a tab"))?;
    // Headless Chrome names itself in its user agent, and some sites refuse it; the plain browser's name is used.
    if let Ok(version) = browser.version().await {
        let _ = page.set_user_agent(version.user_agent.replace("HeadlessChrome", "Chrome")).await;
    }
    page.execute(EnableParams::default()).await.map_err(failed("watch the page"))?;
    let seen: Arc<Mutex<Vec<Seen>>> = Arc::default();
    let mut responses = page.event_listener::<EventResponseReceived>().await.map_err(failed("watch the page"))?;
    let sink = seen.clone();
    let listen = tokio::spawn(async move {
        while let Some(event) = responses.next().await {
            let response = &event.response;
            let seen = Seen {
                url: response.url.clone(),
                mime: Some(response.mime_type.clone()).filter(|mime| !mime.is_empty()),
                size: size_of(response.headers.inner()),
            };
            sink.lock().unwrap_or_else(PoisonError::into_inner).push(seen);
        }
    });

    // A page that never finishes loading still gets scanned with what it loaded.
    let _ = tokio::time::timeout(LOAD_TIMEOUT, page.goto(url)).await;
    let _ = page.evaluate(PLAY).await;
    tokio::time::sleep(SETTLE).await;
    let links: Vec<String> = match page.evaluate(LINKS).await {
        Ok(result) => result.into_value().unwrap_or_default(),
        Err(_) => Vec::new(),
    };
    listen.abort();
    let title = page.get_title().await.ok().flatten().filter(|title| !title.trim().is_empty());
    let address = page.url().await.ok().flatten().unwrap_or_else(|| url.to_string());
    let mut seen = std::mem::take(&mut *seen.lock().unwrap_or_else(PoisonError::into_inner));
    seen.extend(links.into_iter().map(|url| Seen { url, ..Seen::default() }));
    let finds = collect(seen);
    tracing::info!("A deep scan found {} things", finds.len());
    Ok(PageScan { url: address, title, finds })
}

#[cfg(test)]
mod tests {
    use super::{scan, size_of};
    use crate::grab::FindKind;
    use serde_json::json;

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

    #[test]
    fn reads_a_responses_size() {
        assert_eq!(size_of(&json!({"Content-Length": "2048"})), Some(2048));
        assert_eq!(size_of(&json!({"content-range": "bytes 0-1023/99999", "content-length": "1024"})), Some(99999));
        assert_eq!(size_of(&json!({"content-range": "bytes 0-1023/*"})), None);
        assert_eq!(size_of(&json!({})), None);
    }
}

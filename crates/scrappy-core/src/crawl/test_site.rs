//! Test site: a tiny local web server for the crawl and hunt tests, and a request with test limits.

// ── Imports ───────────────────────────────
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use super::{CrawlMode, CrawlRequest};
use crate::grab::FindKind;

// ── Types ─────────────────────────────────
/// A path's content type and body, given the site's address; `None` answers 404.
pub type Route = fn(&str, &str) -> Option<(&'static str, Vec<u8>)>;

// ── Public API ────────────────────────────
/// A crawl request for `url` with roomy limits, no delay, images only, 1 KB smallest.
pub fn request(url: &str, mode: CrawlMode) -> CrawlRequest {
    CrawlRequest { url: url.into(), mode, depth: 3, pages: 50, files: 50, delay_ms: 0, kinds: vec![FindKind::Image], min_kb: 1 }
}

/// Serves `route` on a free local port; returns the site's address, ending in `/`.
pub async fn serve(route: Route) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = format!("http://{}/", listener.local_addr().unwrap());
    let site = address.clone();
    tokio::spawn(async move {
        loop {
            let Ok((mut stream, _)) = listener.accept().await else { return };
            let site = site.clone();
            tokio::spawn(async move {
                let mut buffer = [0u8; 4096];
                let read = stream.read(&mut buffer).await.unwrap_or(0);
                let head = String::from_utf8_lossy(&buffer[..read]);
                let path = head.split_whitespace().nth(1).unwrap_or("/").to_string();
                let (status, kind, body) = match route(&path, &site) {
                    Some((kind, body)) => ("200 OK", kind, body),
                    None => ("404 Not Found", "text/plain", Vec::new()),
                };
                let head = format!(
                    "HTTP/1.1 {status}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = stream.write_all(head.as_bytes()).await;
                let _ = stream.write_all(&body).await;
            });
        }
    });
    address
}

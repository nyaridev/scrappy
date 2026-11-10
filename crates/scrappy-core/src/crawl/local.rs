//! Local: where a mirrored address is saved, and the relative link from one saved file to another (ADR-0008). Pages
//! become `.html` files (`/a/b` → `a/b/index.html`); other hosts' files go under `_hosts/<host>/`; a query is folded
//! into the name, so `a.css?v=2` and `a.css?v=3` stay apart.

// ── Imports ───────────────────────────────
use std::path::{Component, Path, PathBuf};

use reqwest::Url;
use sha2::{Digest, Sha256};

use crate::library::template::sanitize;

// ── Constants ─────────────────────────────
const HOSTS: &str = "_hosts";
const INDEX: &str = "index.html";

// ── Private helpers ───────────────────────
fn decode(segment: &str) -> String {
    let bytes = segment.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        let hex = bytes.get(at + 1..at + 3).and_then(|h| std::str::from_utf8(h).ok());
        match (bytes[at], hex.and_then(|h| u8::from_str_radix(h, 16).ok())) {
            (b'%', Some(byte)) => {
                out.push(byte);
                at += 3;
            }
            (byte, _) => {
                out.push(byte);
                at += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// `name` with a short hash of `query` before its extension: `a.css` → `a-1f2e3d4c.css`.
fn with_query(name: &str, query: &str) -> String {
    let hash: String = format!("{:x}", Sha256::digest(query.as_bytes())).chars().take(8).collect();
    match name.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() => format!("{stem}-{hash}.{ext}"),
        _ => format!("{name}-{hash}"),
    }
}

/// Escapes what a relative link can't hold as it is.
fn link_escape(path: &str) -> String {
    path.chars()
        .map(|c| match c {
            '%' => "%25".to_string(),
            '#' => "%23".to_string(),
            '?' => "%3F".to_string(),
            ' ' => "%20".to_string(),
            c => c.to_string(),
        })
        .collect()
}

// ── Public API ────────────────────────────
/// Where `url` is saved, relative to the mirror's folder. `page` makes it an `.html` file.
pub fn local_path(url: &Url, host: &str, page: bool) -> PathBuf {
    let mut path = PathBuf::new();
    let own = url.host_str().unwrap_or_default();
    if own != host {
        path.push(HOSTS);
        path.push(sanitize(&url.port().map_or_else(|| own.to_string(), |port| format!("{own}_{port}"))));
    }
    let segments: Vec<String> =
        url.path_segments().map(|s| s.map(decode).collect()).unwrap_or_default();
    let (folders, last) = match segments.split_last() {
        Some((last, folders)) => (folders, last.as_str()),
        None => (&[][..], ""),
    };
    for folder in folders.iter().filter(|folder| !folder.is_empty()) {
        path.push(sanitize(folder));
    }
    let html = last.to_ascii_lowercase().ends_with(".html") || last.to_ascii_lowercase().ends_with(".htm");
    let mut name = match (page, last.is_empty(), html) {
        (true, true, _) => INDEX.to_string(),
        (true, false, true) => sanitize(last),
        (true, false, false) => {
            path.push(sanitize(last));
            INDEX.to_string()
        }
        (false, true, _) => "index".to_string(),
        (false, false, _) => sanitize(last),
    };
    if let Some(query) = url.query().filter(|query| !query.is_empty()) {
        name = with_query(&name, query);
    }
    path.push(name);
    path
}

/// The link from the file at `from` to the file at `to`, both relative to the mirror's folder: `a/b/index.html` to
/// `a/style.css` → `../style.css`.
pub fn relative_link(from: &Path, to: &Path) -> String {
    let parts = |path: &Path| -> Vec<String> {
        path.components()
            .filter_map(|c| match c {
                Component::Normal(part) => Some(part.to_string_lossy().into_owned()),
                _ => None,
            })
            .collect()
    };
    let (from, to) = (parts(from.parent().unwrap_or(Path::new(""))), parts(to));
    let shared = from.iter().zip(&to).take_while(|(a, b)| a == b).count();
    let mut link: Vec<String> = vec!["..".to_string(); from.len() - shared];
    link.extend(to[shared..].iter().cloned());
    link_escape(&link.join("/"))
}

#[cfg(test)]
mod tests {
    use super::{local_path, relative_link};
    use reqwest::Url;
    use std::path::{Path, PathBuf};

    fn local(url: &str, page: bool) -> String {
        local_path(&Url::parse(url).unwrap(), "example.com", page).to_string_lossy().replace('\\', "/")
    }

    #[test]
    fn places_pages_and_files() {
        assert_eq!(local("https://example.com/", true), "index.html");
        assert_eq!(local("https://example.com/a/b", true), "a/b/index.html");
        assert_eq!(local("https://example.com/a/b/", true), "a/b/index.html");
        assert_eq!(local("https://example.com/a/page.html", true), "a/page.html");
        assert_eq!(local("https://example.com/list.php", true), "list.php/index.html");
        assert_eq!(local("https://example.com/css/site.css", false), "css/site.css");
        assert_eq!(local("https://cdn.other.net/f/x%20y.woff2", false), "_hosts/cdn.other.net/f/x y.woff2");
        let (v2, v3) = (local("https://example.com/a.css?v=2", false), local("https://example.com/a.css?v=3", false));
        assert!(v2.starts_with("a-") && v2.ends_with(".css") && v2 != v3);
        assert_eq!(local("https://example.com/..%2F..%2Fsecret", false), ".._.._secret");
        // An encoded `..` folder can't step out of the mirror.
        assert!(!local("https://example.com/%2E%2E/%2e%2e/x.png", false).split('/').any(|part| part == ".."));
    }

    #[test]
    fn links_one_file_to_another() {
        let link = |from: &str, to: &str| relative_link(Path::new(from), &PathBuf::from(to));
        assert_eq!(link("index.html", "css/site.css"), "css/site.css");
        assert_eq!(link("a/b/index.html", "a/style.css"), "../style.css");
        assert_eq!(link("a/b/index.html", "index.html"), "../../index.html");
        assert_eq!(link("a/index.html", "a/c d#1.png"), "c%20d%231.png");
    }
}

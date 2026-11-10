//! Links: the links in a page's HTML, made absolute against the page: its files for a crawl (images with lazy
//! `data-src` and `srcset`, media and posters, `og:image`, anchors to files, inline `url()`), or its assets for a
//! mirror (stylesheets, scripts, icons, images, media); and the `url()` and `@import` links in CSS (ADR-0008).

// ── Imports ───────────────────────────────
use std::cell::RefCell;

use lol_html::{HtmlRewriter, Settings, element};
use reqwest::Url;

// ── Private helpers ───────────────────────
/// The links in a `srcset`: `a.jpg 1x, b.jpg 2x` → `a.jpg`, `b.jpg`.
fn srcset(value: &str) -> impl Iterator<Item = &str> {
    value.split(',').filter_map(|part| part.split_whitespace().next())
}

/// `<link rel>` values whose `href` a mirror saves.
const ASSET_RELS: &[&str] = &["stylesheet", "icon", "shortcut", "apple-touch-icon", "preload", "manifest"];

/// The `url(...)` links in a style.
pub(super) fn style_urls(style: &str) -> Vec<&str> {
    style
        .split("url(")
        .skip(1)
        .filter_map(|rest| rest.split_once(')'))
        .map(|(inside, _)| inside.trim().trim_matches(['"', '\'']))
        .collect()
}

/// The `@import "a.css"` links in CSS (the `@import url(a.css)` form is a `url()`).
fn css_imports(css: &str) -> Vec<&str> {
    css.split("@import")
        .skip(1)
        .filter_map(|rest| {
            let rest = rest.trim_start();
            let quote = rest.chars().next().filter(|c| *c == '"' || *c == '\'')?;
            rest[1..].split(quote).next()
        })
        .collect()
}

/// The links in `html`: its files (`assets` false) or the assets a mirror saves (`assets` true).
fn collect(page: &Url, html: &str, assets: bool) -> Vec<String> {
    let found = RefCell::new(Vec::<String>::new());
    let add = |link: &str| {
        let link = link.trim();
        if link.is_empty() || link.starts_with("data:") {
            return;
        }
        if let Ok(url) = page.join(link)
            && matches!(url.scheme(), "http" | "https")
        {
            let url = String::from(url);
            let mut found = found.borrow_mut();
            if !found.contains(&url) {
                found.push(url);
            }
        }
    };
    let attribute = |names: &'static [&'static str]| {
        move |el: &mut lol_html::html_content::Element| {
            for name in names {
                if let Some(value) = el.get_attribute(name) {
                    if name.ends_with("srcset") {
                        srcset(&value).for_each(add);
                    } else {
                        add(&value);
                    }
                }
            }
            Ok(())
        }
    };
    let mut handlers = vec![
        element!("img", attribute(&["src", "data-src", "data-original", "srcset", "data-srcset"])),
        element!("source", attribute(&["src", "srcset"])),
        element!("video", attribute(&["src", "poster"])),
        element!("audio", attribute(&["src"])),
        element!("[style*='url(']", |el| {
            if let Some(style) = el.get_attribute("style") {
                style_urls(&style).into_iter().for_each(add);
            }
            Ok(())
        }),
    ];
    if assets {
        handlers.push(element!("script[src]", attribute(&["src"])));
        handlers.push(element!("link[href]", |el| {
            let rel = el.get_attribute("rel").unwrap_or_default().to_ascii_lowercase();
            if rel.split_whitespace().any(|rel| ASSET_RELS.contains(&rel))
                && let Some(href) = el.get_attribute("href")
            {
                add(&href);
            }
            Ok(())
        }));
    } else {
        handlers.push(element!("a[href]", attribute(&["href"])));
        handlers.push(element!(r#"meta[property="og:image"], meta[name="twitter:image"]"#, attribute(&["content"])));
    }
    let mut rewriter =
        HtmlRewriter::new(Settings { element_content_handlers: handlers, ..Settings::new() }, |_: &[u8]| {});
    // A page lol_html can't parse to the end still gives what it read before.
    let _ = rewriter.write(html.as_bytes());
    let _ = rewriter.end();
    found.into_inner()
}

// ── Public API ────────────────────────────
/// The file links in `html`, absolute against `page`, web links only, each once, in the order they appear.
pub fn file_links(page: &Url, html: &str) -> Vec<String> {
    collect(page, html, false)
}

/// The assets a mirror saves with the page: stylesheets, scripts, icons, images, media and inline `url()`s.
pub fn asset_links(page: &Url, html: &str) -> Vec<String> {
    collect(page, html, true)
}

/// The `url()` and `@import` links in CSS, absolute against the stylesheet's own address.
pub fn css_links(base: &Url, css: &str) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    for link in style_urls(css).into_iter().chain(css_imports(css)) {
        let Ok(url) = base.join(link.trim()) else { continue };
        let url = String::from(url);
        if url.starts_with("http") && !found.contains(&url) {
            found.push(url);
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::{asset_links, css_links, file_links};
    use reqwest::Url;

    #[test]
    fn finds_every_kind_of_file_link() {
        let page = Url::parse("https://example.com/gallery/page.html").unwrap();
        let html = r#"<html><head><meta property="og:image" content="/og.jpg"></head><body>
            <img src="a.jpg" srcset="a-small.jpg 1x, https://cdn.example.com/a-2x.jpg 2x">
            <img data-src="lazy.webp" src="data:image/gif;base64,R0lGOD">
            <picture><source srcset="b.avif"></picture>
            <video src="clip.mp4" poster="poster.png"></video>
            <a href="full/a.jpg">full</a><a href="mailto:x@example.com">mail</a>
            <div style="background: url('bg.png') no-repeat"></div>
            <img src="a.jpg">
        </body></html>"#;
        let links = file_links(&page, html);
        assert_eq!(
            links,
            [
                "https://example.com/og.jpg",
                "https://example.com/gallery/a.jpg",
                "https://example.com/gallery/a-small.jpg",
                "https://cdn.example.com/a-2x.jpg",
                "https://example.com/gallery/lazy.webp",
                "https://example.com/gallery/b.avif",
                "https://example.com/gallery/clip.mp4",
                "https://example.com/gallery/poster.png",
                "https://example.com/gallery/full/a.jpg",
                "https://example.com/gallery/bg.png",
            ]
        );
    }

    #[test]
    fn finds_a_pages_assets_and_its_css_links() {
        let page = Url::parse("https://example.com/a/").unwrap();
        let html = r#"<link rel="stylesheet" href="/s.css"><link rel="canonical" href="/a/">
            <link rel="shortcut icon" href="fav.ico"><script src="app.js"></script><script>inline()</script>
            <a href="/next">next</a><img src="i.png">"#;
        assert_eq!(
            asset_links(&page, html),
            [
                "https://example.com/s.css",
                "https://example.com/a/fav.ico",
                "https://example.com/a/app.js",
                "https://example.com/a/i.png",
            ]
        );
        let css = r#"@import "base.css"; @import url('print.css'); body { background: url(img/bg.png) }
            @font-face { src: url("https://fonts.example.net/f.woff2") format("woff2"), url(data:font/woff2;base64,AA) }"#;
        assert_eq!(
            css_links(&Url::parse("https://example.com/css/site.css").unwrap(), css),
            [
                "https://example.com/css/print.css",
                "https://example.com/css/img/bg.png",
                "https://fonts.example.net/f.woff2",
                "https://example.com/css/base.css",
            ]
        );
    }
}

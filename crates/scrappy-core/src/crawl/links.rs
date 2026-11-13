//! Links: what a mirror reads from a page, made absolute against it (ADR-0008, ADR-0033): the assets it saves with
//! the page (stylesheets, scripts, icons), the images and videos it lists to pick from (`img` with lazy `data-src` and
//! `srcset`, `video` and its poster, `source`, inline `url()`, links to image and video files), and the `url()` and
//! `@import` links in CSS.

// ── Imports ───────────────────────────────
use std::borrow::Cow;
use std::cell::RefCell;

use lol_html::html_content::Element;
use lol_html::{ElementContentHandlers, HtmlRewriter, Selector, Settings, element};
use reqwest::Url;

use crate::grab::{FindKind, classify};

// ── Constants ─────────────────────────────
/// `<link rel>` values whose `href` a mirror saves.
const ASSET_RELS: &[&str] = &["stylesheet", "icon", "shortcut", "apple-touch-icon", "preload", "manifest"];

// ── Types ─────────────────────────────────
type Handlers<'h> = Vec<(Cow<'h, Selector>, ElementContentHandlers<'h>)>;

/// The links met in a page, each once, in order, with what the element says they are (`None`: read the link).
struct Links<'a> {
    page: &'a Url,
    found: RefCell<Vec<(String, Option<FindKind>)>>,
}

// ── Private helpers ───────────────────────
/// The links in a `srcset`: `a.jpg 1x, b.jpg 2x` → `a.jpg`, `b.jpg`.
fn srcset(value: &str) -> impl Iterator<Item = &str> {
    value.split(',').filter_map(|part| part.split_whitespace().next())
}

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

impl<'a> Links<'a> {
    fn new(page: &'a Url) -> Self {
        Self { page, found: RefCell::new(Vec::new()) }
    }

    fn add(&self, link: &str, kind: Option<FindKind>) {
        let link = link.trim();
        if link.is_empty() || link.starts_with("data:") {
            return;
        }
        if let Ok(url) = self.page.join(link)
            && matches!(url.scheme(), "http" | "https")
        {
            let url = String::from(url);
            let mut found = self.found.borrow_mut();
            if !found.iter().any(|(seen, _)| *seen == url) {
                found.push((url, kind));
            }
        }
    }

    /// Adds the element's `names` attributes (a `srcset` link by link).
    fn attributes(&self, el: &Element, names: &[&str], kind: Option<FindKind>) {
        for name in names {
            if let Some(value) = el.get_attribute(name) {
                if name.ends_with("srcset") {
                    srcset(&value).for_each(|link| self.add(link, kind));
                } else {
                    self.add(&value, kind);
                }
            }
        }
    }

    /// Runs `handlers` over `html`. A page lol_html can't parse to the end still gives what it read before.
    fn read(&self, html: &str, handlers: Handlers<'_>) -> Vec<(String, Option<FindKind>)> {
        let mut rewriter =
            HtmlRewriter::new(Settings { element_content_handlers: handlers, ..Settings::new() }, |_: &[u8]| {});
        let _ = rewriter.write(html.as_bytes());
        let _ = rewriter.end();
        self.found.take()
    }
}

// ── Public API ────────────────────────────
/// The assets a mirror saves with the page: stylesheets, scripts and icons.
pub fn asset_links(page: &Url, html: &str) -> Vec<String> {
    let links = Links::new(page);
    let handlers = vec![
        element!("script[src]", |el| {
            links.attributes(el, &["src"], None);
            Ok(())
        }),
        element!("link[href]", |el| {
            let rel = el.get_attribute("rel").unwrap_or_default().to_ascii_lowercase();
            if rel.split_whitespace().any(|rel| ASSET_RELS.contains(&rel))
                && let Some(href) = el.get_attribute("href")
            {
                links.add(&href, None);
            }
            Ok(())
        }),
    ];
    links.read(html, handlers).into_iter().map(|(url, _)| url).collect()
}

/// The images and videos the page shows or links to, each once, in the order they appear.
pub fn media_links(page: &Url, html: &str) -> Vec<(String, FindKind)> {
    let (image, video) = (Some(FindKind::Image), Some(FindKind::Video));
    let links = Links::new(page);
    let handlers = vec![
        element!("img", |el| {
            links.attributes(el, &["src", "data-src", "data-original", "srcset", "data-srcset"], image);
            Ok(())
        }),
        element!("video", |el| {
            links.attributes(el, &["src"], video);
            links.attributes(el, &["poster"], image);
            Ok(())
        }),
        element!("source, a[href]", |el| {
            links.attributes(el, &["src", "srcset", "href"], None);
            Ok(())
        }),
        element!("[style*='url(']", |el| {
            if let Some(style) = el.get_attribute("style") {
                style_urls(&style).into_iter().for_each(|link| links.add(link, image));
            }
            Ok(())
        }),
    ];
    links
        .read(html, handlers)
        .into_iter()
        .filter_map(|(url, kind)| {
            let kind = kind.or_else(|| classify(&url, None))?;
            matches!(kind, FindKind::Image | FindKind::Video).then_some((url, kind))
        })
        .collect()
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
    use super::{asset_links, css_links, media_links};
    use crate::grab::FindKind;
    use reqwest::Url;

    #[test]
    fn finds_a_pages_assets_and_its_css_links() {
        let page = Url::parse("https://example.com/a/").unwrap();
        let html = r#"<link rel="stylesheet" href="/s.css"><link rel="canonical" href="/a/">
            <link rel="shortcut icon" href="fav.ico"><script src="app.js"></script><script>inline()</script>
            <a href="/next">next</a><img src="i.png">"#;
        assert_eq!(
            asset_links(&page, html),
            ["https://example.com/s.css", "https://example.com/a/fav.ico", "https://example.com/a/app.js"]
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

    #[test]
    fn lists_the_images_and_videos_a_page_shows() {
        let page = Url::parse("https://example.com/gallery/page.html").unwrap();
        let html = r#"<img src="a.jpg" srcset="a-small.jpg 1x, https://cdn.example.com/a-2x.jpg 2x">
            <img data-src="/photo?id=3" src="data:image/gif;base64,R0lGOD">
            <video src="clip.mp4" poster="poster.png"><source src="clip.webm"></video>
            <a href="full/a.jpg">full</a><a href="/next">next</a><a href="doc.pdf">doc</a><audio src="song.mp3"></audio>
            <div style="background: url('bg.png') no-repeat"></div><img src="a.jpg">"#;
        let (image, video) = (FindKind::Image, FindKind::Video);
        let want = [
            ("https://example.com/gallery/a.jpg", image),
            ("https://example.com/gallery/a-small.jpg", image),
            ("https://cdn.example.com/a-2x.jpg", image),
            ("https://example.com/photo?id=3", image),
            ("https://example.com/gallery/clip.mp4", video),
            ("https://example.com/gallery/poster.png", image),
            ("https://example.com/gallery/clip.webm", video),
            ("https://example.com/gallery/full/a.jpg", image),
            ("https://example.com/gallery/bg.png", image),
        ];
        assert_eq!(media_links(&page, html), want.map(|(url, kind)| (url.to_string(), kind)));
    }
}

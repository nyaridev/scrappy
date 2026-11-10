//! Content: the one finder Hunt uses on every page. It parses the page, drops what is never content (scripts, menus,
//! headers, footers, sidebars, ads…), scores the remaining blocks by their paragraphs and pictures, takes the best one
//! as the content, and lists its images, minus logos, icons and avatars, by the kinds the user switched on
//! (ADR-0013).

// ── Imports ───────────────────────────────
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::LazyLock;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use html5ever::serialize::{SerializeOpts, TraversalScope};
use html5ever::tendril::TendrilSink;
use html5ever::tree_builder::TreeBuilderOpts;
use html5ever::{ParseOpts, parse_document};
use markup5ever_rcdom::{Handle, Node, NodeData, RcDom, SerializableHandle};
use regex::Regex;
use reqwest::Url;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::grab::{FindKind, classify};

// ── Constants ─────────────────────────────
/// Never content.
const JUNK_TAGS: &[&str] = &[
    "script", "style", "template", "iframe", "nav", "aside", "form", "button", "select", "textarea", "dialog", "object",
    "embed", "canvas", "video", "audio",
];
/// Page chrome, unless inside an article or the main block (an article's header holds its title picture).
const EDGE_TAGS: &[&str] = &["header", "footer"];
const JUNK_ROLES: &[&str] =
    &["navigation", "banner", "contentinfo", "complementary", "menu", "menubar", "search", "dialog", "alert", "toolbar"];
/// Blocks whose text counts as content.
const PARAGRAPHS: &[&str] = &["p", "pre", "td", "blockquote", "dd", "figcaption"];
/// Attributes that hold a full-size picture.
const FULL_SIZE: &[&str] =
    &["data-orig-file", "data-large-file", "data-full", "data-full-src", "data-hi-res-src", "data-zoom-image"];
const SETS: &[&str] = &["srcset", "data-srcset", "data-lazy-srcset"];
/// Attributes lazy loaders keep the real picture in.
const LAZY: &[&str] = &["data-src", "data-original", "data-lazy-src", "data-lazy", "data-url", "data-image"];
/// Pictures declared smaller than this many pixels a side are chrome.
const MIN_SIDE: u32 = 100;
/// Shorter paragraphs don't count.
const MIN_TEXT: usize = 25;
/// A best block scoring under this isn't clearly the content; the whole cleaned page is used.
const MIN_SCORE: f64 = 20.0;
/// Points a content-sized picture gives its block, so galleries win without text.
const PICTURE_POINTS: f64 = 3.0;

static NEGATIVE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(^|[\s_-])(nav|navbar|navigation|menu|header|footer|sidebar|comments?|share|sharing|social|related|recommended|promo|advert|advertisement|ads?|banner|sponsor|sponsored|cookie|consent|popup|modal|breadcrumbs?|widget|toolbar|subscribe|newsletter|login|signup|masthead|pagination|pager|outbrain|taboola)([\s_-]|$)")
        .expect("a valid pattern")
});
static POSITIVE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)article|content|entry|post|main|body|story|gallery|blog|wiki|mw-parser|prose").expect("a valid pattern")
});
static CHROME: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(^|[^a-z])(logo|logos|icon|icons|avatar|avatars|emoji|emoticon|sprite|spinner|loader|loading|badge|pixel|spacer|blank|placeholder|tracking|gravatar|favicon|flag|flags)([^a-z]|$)")
        .expect("a valid pattern")
});
static CSS_URL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"url\(\s*['"]?([^'")]+?)['"]?\s*\)"#).expect("a valid pattern"));

// ── Types ─────────────────────────────────
/// A kind of image data in a page; each has its own switch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ImageKind {
    /// `<img src>`.
    Img,
    /// Lazy and high-resolution versions: `srcset`, `data-src` and the like, `<picture>` sources; the largest wins.
    Responsive,
    /// Links around content that point at an image file, usually the full size.
    Linked,
    /// CSS `background-image` in the content's style attributes.
    Background,
    /// The page's preview picture: `og:image`, `twitter:image`.
    Preview,
    /// `data:` images and inline `<svg>`.
    Inline,
}

/// An image the finder kept: a web address, or a `data:` URL for an inline one.
#[derive(Debug, Clone, PartialEq)]
pub struct Found {
    pub url: String,
    pub kind: ImageKind,
}

/// Text counts of an element and what's inside it.
#[derive(Debug, Clone, Copy, Default)]
struct Stats {
    /// Characters that aren't spaces.
    text: usize,
    /// Of those, the ones inside links.
    links: usize,
    commas: usize,
}

/// The parsed page and what the finder learns of it.
struct Page {
    base: Url,
    stats: HashMap<*const Node, Stats>,
    total: usize,
}

// ── Private helpers ───────────────────────
fn tag(node: &Handle) -> Option<&str> {
    match &node.data {
        NodeData::Element { name, .. } => Some(&*name.local),
        _ => None,
    }
}

fn attr(node: &Handle, key: &str) -> Option<String> {
    match &node.data {
        NodeData::Element { attrs, .. } => attrs.borrow().iter().find(|a| &*a.name.local == key).map(|a| a.value.to_string()),
        _ => None,
    }
}

fn id_of(node: &Handle) -> *const Node {
    Rc::as_ptr(node)
}

/// Counts every element's text, bottom up.
fn measure(node: &Handle, stats: &mut HashMap<*const Node, Stats>) -> Stats {
    let mut own = Stats::default();
    match &node.data {
        NodeData::Text { contents } => {
            let text = contents.borrow();
            own.text = text.chars().filter(|c| !c.is_whitespace()).count();
            own.commas = text.matches(',').count();
        }
        NodeData::Element { .. } if matches!(tag(node), Some("script" | "style" | "template")) => {}
        _ => {
            for child in node.children.borrow().iter() {
                let inner = measure(child, stats);
                own.text += inner.text;
                own.links += inner.links;
                own.commas += inner.commas;
            }
            if tag(node) == Some("a") {
                own.links = own.text;
            }
        }
    }
    if tag(node).is_some() {
        stats.insert(id_of(node), own);
    }
    own
}

/// The element's class and id together.
fn names(node: &Handle) -> String {
    format!("{} {}", attr(node, "class").unwrap_or_default(), attr(node, "id").unwrap_or_default())
}

/// Never content: chrome by tag, role or name. A block named like chrome still counts when it holds half the page's
/// text (a wrapper named `has-sidebar`).
fn is_junk(page: &Page, node: &Handle, in_article: bool) -> bool {
    let Some(name) = tag(node) else { return false };
    if matches!(name, "html" | "body" | "main" | "article") {
        return false;
    }
    if JUNK_TAGS.contains(&name) || (EDGE_TAGS.contains(&name) && !in_article) {
        return true;
    }
    if attr(node, "role").is_some_and(|role| JUNK_ROLES.contains(&role.to_ascii_lowercase().as_str()))
        || attr(node, "aria-hidden").as_deref() == Some("true")
        || attr(node, "hidden").is_some()
    {
        return true;
    }
    let names = names(node);
    let text = page.stats.get(&id_of(node)).map_or(0, |stats| stats.text);
    NEGATIVE.is_match(&names) && !POSITIVE.is_match(&names) && text * 2 <= page.total
}

/// A picture of the page's chrome: named like it, or declared small.
fn is_chrome(node: &Handle) -> bool {
    let named = ["src", "class", "id", "alt"].iter().filter_map(|key| attr(node, key)).any(|value| CHROME.is_match(&value));
    let side = |key: &str| attr(node, key).and_then(|value| value.trim().trim_end_matches("px").parse::<f64>().ok());
    named || [side("width"), side("height")].into_iter().flatten().any(|side| side < f64::from(MIN_SIDE))
}

/// A block's starting score, from its tag and its name.
fn initial(node: &Handle) -> f64 {
    let by_tag = match tag(node) {
        Some("article" | "main") => 10.0,
        Some("div") => 5.0,
        Some("pre" | "td" | "blockquote") => 3.0,
        Some("ol" | "ul" | "dl" | "form") => -3.0,
        Some("h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "th") => -5.0,
        _ => 0.0,
    };
    let names = names(node);
    let by_name = if POSITIVE.is_match(&names) { 25.0 } else { 0.0 } - if NEGATIVE.is_match(&names) { 25.0 } else { 0.0 };
    by_tag + by_name
}

/// Scores blocks: each paragraph and content-sized picture gives points to the blocks around it, less the further out.
fn score(page: &Page, node: &Handle, ancestors: &mut Vec<Handle>, in_article: bool, scores: &mut HashMap<*const Node, (Handle, f64)>) {
    let Some(name) = tag(node) else {
        node.children.borrow().iter().for_each(|child| score(page, child, ancestors, in_article, scores));
        return;
    };
    if is_junk(page, node, in_article) {
        return;
    }
    let stats = page.stats.get(&id_of(node)).copied().unwrap_or_default();
    let points = match name {
        _ if PARAGRAPHS.contains(&name) && stats.text >= MIN_TEXT => Some(1.0 + stats.commas as f64 + (stats.text as f64 / 100.0).min(3.0)),
        "img" if !is_chrome(node) => Some(PICTURE_POINTS),
        _ => None,
    };
    if let Some(points) = points {
        for (level, block) in ancestors.iter().rev().take(5).enumerate() {
            let share = match level {
                0 => 1.0,
                1 => 2.0,
                level => level as f64 * 3.0,
            };
            scores.entry(id_of(block)).or_insert_with(|| (block.clone(), initial(block))).1 += points / share;
        }
    }
    let in_article = in_article || matches!(name, "article" | "main");
    ancestors.push(node.clone());
    node.children.borrow().iter().for_each(|child| score(page, child, ancestors, in_article, scores));
    ancestors.pop();
}

/// The content: the best block and its siblings that score close to it, or the whole page when nothing stands out.
fn content_roots(page: &Page, document: &Handle) -> Vec<Handle> {
    let mut scores = HashMap::new();
    score(page, document, &mut Vec::new(), false, &mut scores);
    let link_density = |node: &Handle| {
        let stats = page.stats.get(&id_of(node)).copied().unwrap_or_default();
        if stats.text == 0 { 0.0 } else { stats.links as f64 / stats.text as f64 }
    };
    let finals: HashMap<*const Node, (Handle, f64)> =
        scores.into_iter().map(|(key, (node, score))| (key, (node.clone(), score * (1.0 - link_density(&node))))).collect();
    let Some((best, top)) = finals.values().max_by(|a, b| a.1.total_cmp(&b.1)).cloned() else { return vec![document.clone()] };
    if top < MIN_SCORE {
        return vec![document.clone()];
    }
    let parent = best.parent.take();
    best.parent.set(parent.clone());
    let Some(parent) = parent.and_then(|weak| weak.upgrade()) else { return vec![best] };
    let close = top * 0.2;
    parent
        .children
        .borrow()
        .iter()
        .filter(|child| Rc::ptr_eq(child, &best) || finals.get(&id_of(child)).is_some_and(|(_, score)| *score >= close.max(10.0)))
        .cloned()
        .collect()
}

/// The largest candidate in a `srcset`: by width (`800w`), else density (`2x`).
fn largest(srcset: &str) -> Option<String> {
    srcset
        .split(", ")
        .filter_map(|part| {
            let mut words = part.split_whitespace();
            let link = words.next()?.trim_end_matches(',');
            let size = words.next().and_then(|size| {
                let (number, unit) = size.split_at(size.len().saturating_sub(1));
                let number: f64 = number.parse().ok()?;
                Some(if unit == "x" { number * 1000.0 } else { number })
            });
            Some((link.to_string(), size.unwrap_or(1.0)))
        })
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(link, _)| link)
}

/// A picture's best lazy or high-resolution address: a full-size attribute, else the largest `srcset` candidate, else
/// a lazy loader's attribute.
fn responsive(node: &Handle) -> Option<String> {
    let full = FULL_SIZE.iter().find_map(|key| attr(node, key));
    let set = || SETS.iter().find_map(|key| attr(node, key)).and_then(|set| largest(&set));
    let lazy = || LAZY.iter().find_map(|key| attr(node, key));
    full.or_else(set).or_else(lazy).filter(|link| !link.trim().is_empty() && !link.starts_with("data:"))
}

/// An inline `<svg>` as a `data:` URL that stands on its own.
fn svg_data(node: &Handle) -> Option<String> {
    let mut out = Vec::new();
    let options = SerializeOpts { traversal_scope: TraversalScope::IncludeNode, ..SerializeOpts::default() };
    html5ever::serialize(&mut out, &SerializableHandle::from(node.clone()), options).ok()?;
    let mut svg = String::from_utf8(out).ok()?;
    if !svg.contains("xmlns=") {
        svg = svg.replacen("<svg", r#"<svg xmlns="http://www.w3.org/2000/svg""#, 1);
    }
    Some(format!("data:image/svg+xml;base64,{}", STANDARD.encode(svg)))
}

/// Collects the content's images of the wanted kinds, in page order.
fn collect(page: &Page, node: &Handle, in_article: bool, kinds: &[ImageKind], found: &mut Vec<Found>) {
    let Some(name) = tag(node) else {
        node.children.borrow().iter().for_each(|child| collect(page, child, in_article, kinds, found));
        return;
    };
    if is_junk(page, node, in_article) {
        return;
    }
    let on = |kind: ImageKind| kinds.contains(&kind);
    let mut add = |link: &str, kind: ImageKind| {
        let link = link.trim();
        let url = if link.starts_with("data:image/") {
            on(ImageKind::Inline).then(|| link.to_string())
        } else {
            page.base.join(link).ok().filter(|url| matches!(url.scheme(), "http" | "https")).map(String::from)
        };
        if let Some(url) = url
            && !found.iter().any(|known| known.url == url)
        {
            found.push(Found { url, kind });
        }
    };
    match name {
        "img" if !is_chrome(node) => match responsive(node).filter(|_| on(ImageKind::Responsive)) {
            Some(link) => add(&link, ImageKind::Responsive),
            None => {
                let src = attr(node, "src").unwrap_or_default();
                let kind = if src.starts_with("data:") { ImageKind::Inline } else { ImageKind::Img };
                if on(kind) {
                    add(&src, kind);
                }
            }
        },
        "source" if on(ImageKind::Responsive) => {
            if let Some(link) = attr(node, "srcset").and_then(|set| largest(&set)) {
                add(&link, ImageKind::Responsive);
            }
        }
        "a" if on(ImageKind::Linked) => {
            if let Some(href) = attr(node, "href")
                && page.base.join(&href).is_ok_and(|url| classify(url.as_str(), None) == Some(FindKind::Image))
            {
                add(&href, ImageKind::Linked);
            }
        }
        "svg" => {
            if on(ImageKind::Inline) && !is_chrome(node) && let Some(data) = svg_data(node) {
                add(&data, ImageKind::Inline);
            }
            return;
        }
        _ => {}
    }
    if on(ImageKind::Background) && let Some(style) = attr(node, "style") {
        for link in CSS_URL.captures_iter(&style).map(|found| found[1].to_string()) {
            add(&link, ImageKind::Background);
        }
    }
    let in_article = in_article || matches!(name, "article" | "main");
    node.children.borrow().iter().for_each(|child| collect(page, child, in_article, kinds, found));
}

/// Every element under `node` with tag `name`, in order.
fn find_all(node: &Handle, name: &str, out: &mut Vec<Handle>) {
    if tag(node) == Some(name) {
        out.push(node.clone());
    }
    node.children.borrow().iter().for_each(|child| find_all(child, name, out));
}

// ── Public API ────────────────────────────
/// The images in the content of the page at `url`, of the `kinds` switched on, each once, in page order. The
/// preview picture comes first; inline images come as `data:` URLs.
pub fn images(url: &Url, html: &str, kinds: &[ImageKind]) -> Vec<Found> {
    let options = ParseOpts { tree_builder: TreeBuilderOpts { scripting_enabled: false, ..TreeBuilderOpts::default() }, ..ParseOpts::default() };
    let dom = parse_document(RcDom::default(), options).one(html);
    let mut bases = Vec::new();
    find_all(&dom.document, "base", &mut bases);
    let base = bases.first().and_then(|base| attr(base, "href")).and_then(|href| url.join(&href).ok()).unwrap_or_else(|| url.clone());
    let mut stats = HashMap::new();
    let total = measure(&dom.document, &mut stats).text;
    let page = Page { base, stats, total };

    let mut found = Vec::new();
    if kinds.contains(&ImageKind::Preview) {
        let mut metas = Vec::new();
        find_all(&dom.document, "meta", &mut metas);
        for meta in metas {
            let key = attr(&meta, "property").or_else(|| attr(&meta, "name")).unwrap_or_default().to_ascii_lowercase();
            if matches!(key.as_str(), "og:image" | "og:image:url" | "og:image:secure_url" | "twitter:image")
                && let Some(link) = attr(&meta, "content").and_then(|link| page.base.join(link.trim()).ok())
                && matches!(link.scheme(), "http" | "https")
                && !found.iter().any(|known: &Found| known.url == link.as_str())
            {
                found.push(Found { url: link.into(), kind: ImageKind::Preview });
            }
        }
    }
    for root in content_roots(&page, &dom.document) {
        collect(&page, &root, false, kinds, &mut found);
    }
    found
}

#[cfg(test)]
mod tests {
    use super::{Found, ImageKind, images, largest};
    use reqwest::Url;

    const ALL: &[ImageKind] = &[
        ImageKind::Img,
        ImageKind::Responsive,
        ImageKind::Linked,
        ImageKind::Background,
        ImageKind::Preview,
        ImageKind::Inline,
    ];

    const ARTICLE: &str = r#"<html><head><meta property="og:image" content="/share.jpg"></head><body>
        <header class="site-header"><a href="/"><img src="/brand.png" alt="Site"></a>
          <nav><ul><li><a href="/a">A</a></li><li><img src="/nav-photo.jpg"></li></ul></nav></header>
        <div class="layout has-sidebar">
          <div id="main-content" class="post-body">
            <h1>Metin2 dragons</h1>
            <p>Dragons are the hardest bosses in the game, and they drop the rarest items, which players trade for weeks.</p>
            <figure><img src="/img/dragon-small.jpg" srcset="/img/dragon-small.jpg 400w, /img/dragon-big.jpg 1600w"></figure>
            <p>The red dragon lives in the fire land, past the desert, behind a gate that opens once a day at noon.</p>
            <img src="data:image/gif;base64,R0lGODlhAQABAAAAACw=" data-src="/img/lazy.jpg">
            <a href="/img/full.png"><img src="/img/thumb.png" width="300" height="200"></a>
            <img src="/img/share-icon.png" width="16" height="16">
            <div style="background-image: url('/img/back.webp')"><p>A map of the fire land, drawn by players, with every spawn point and the safe paths.</p></div>
            <svg width="400" height="300" viewBox="0 0 4 3"><rect width="4" height="3"/></svg>
            <svg class="icon" width="16" height="16"></svg>
          </div>
          <aside class="sidebar"><img src="/ad-banner.jpg"></aside>
          <div class="ads"><img src="/ad.jpg"></div>
        </div>
        <footer><img src="/footer-logo.png"></footer></body></html>"#;

    fn urls(found: &[Found]) -> Vec<String> {
        found.iter().map(|found| found.url.replace("https://example.com", "")).collect()
    }

    #[test]
    fn finds_the_content_images_of_an_article() {
        let page = Url::parse("https://example.com/news/dragons").unwrap();
        let found = images(&page, ARTICLE, ALL);
        let mut kept = urls(&found);
        let svg = kept.iter().position(|url| url.starts_with("data:image/svg+xml")).expect("the inline drawing");
        kept.remove(svg);
        assert_eq!(kept, ["/share.jpg", "/img/dragon-big.jpg", "/img/lazy.jpg", "/img/full.png", "/img/thumb.png", "/img/back.webp"]);
        assert_eq!(found.iter().filter(|found| found.url.starts_with("data:")).count(), 1, "the 16px icon isn't kept");
    }

    #[test]
    fn follows_the_switches() {
        let page = Url::parse("https://example.com/news/dragons").unwrap();
        let plain = urls(&images(&page, ARTICLE, &[ImageKind::Img]));
        assert_eq!(plain, ["/img/dragon-small.jpg", "/img/thumb.png"]);
        let linked = urls(&images(&page, ARTICLE, &[ImageKind::Linked, ImageKind::Background]));
        assert_eq!(linked, ["/img/full.png", "/img/back.webp"]);
    }

    #[test]
    fn finds_a_gallery_without_text() {
        let html = r#"<body><nav><img src="/n.jpg"></nav><div class="grid">
            <a href="/p/1"><img src="/g/1.jpg"></a><a href="/p/2"><img src="/g/2.jpg"></a><a href="/p/3"><img src="/g/3.jpg"></a>
            <a href="/p/4"><img src="/g/4.jpg"></a><a href="/p/5"><img src="/g/5.jpg"></a><a href="/p/6"><img src="/g/6.jpg"></a>
            <a href="/p/7"><img src="/g/7.jpg"></a></div><div class="footer"><img src="/f.jpg"></div></body>"#;
        let found = urls(&images(&Url::parse("https://example.com/").unwrap(), html, &[ImageKind::Img]));
        assert_eq!(found, (1..=7).map(|n| format!("/g/{n}.jpg")).collect::<Vec<_>>());
    }

    #[test]
    fn falls_back_to_the_whole_cleaned_page() {
        let html = r#"<body><img src="/a.jpg"><nav><img src="/menu.jpg"></nav></body>"#;
        assert_eq!(urls(&images(&Url::parse("https://example.com/").unwrap(), html, &[ImageKind::Img])), ["/a.jpg"]);
        assert_eq!(largest("a.jpg 1x, b.jpg 2x").as_deref(), Some("b.jpg"));
        assert_eq!(largest("https://c.net/w_300,h_200/a.jpg 300w, https://c.net/w_900,h_600/a.jpg 900w").as_deref(), Some("https://c.net/w_900,h_600/a.jpg"));
    }
}

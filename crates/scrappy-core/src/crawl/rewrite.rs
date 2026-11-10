//! Rewrite: points a saved page's and stylesheet's links at the mirror's copies by relative path (`href`, `src`,
//! `srcset`, `poster`, lazy `data-src`, CSS `url()` and `@import`, in attributes, `<style>` and CSS files). A link to
//! anything the mirror didn't save stays as it was (ADR-0008).

// ── Imports ───────────────────────────────
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use lol_html::html_content::ContentType;
use lol_html::{HtmlRewriter, Settings, element, text};
use reqwest::Url;

use super::local::relative_link;

// ── Constants ─────────────────────────────
const SINGLE: &[&str] = &["href", "src", "poster", "data-src"];
const SETS: &[&str] = &["srcset", "data-srcset"];

// ── Types ─────────────────────────────────
/// What a mirror saved: each address, without its fragment, and where it went in the mirror's folder.
pub type Saved = HashMap<String, PathBuf>;

// ── Private helpers ───────────────────────
/// The relative link to `link`'s saved copy, from the file at `from`, keeping its `#fragment`.
fn target(saved: &Saved, base: &Url, from: &Path, link: &str) -> Option<String> {
    let link = link.trim();
    if link.is_empty() || link.starts_with('#') {
        return None;
    }
    let mut url = base.join(link).ok()?;
    let fragment = url.fragment().map(|fragment| format!("#{fragment}")).unwrap_or_default();
    url.set_fragment(None);
    saved.get(url.as_str()).map(|local| relative_link(from, local) + &fragment)
}

fn srcset(value: &str, swap: impl Fn(&str) -> Option<String>) -> String {
    value
        .split(',')
        .map(|part| {
            let part = part.trim();
            let (link, size) = part.split_once(char::is_whitespace).unwrap_or((part, ""));
            let link = swap(link).unwrap_or_else(|| link.to_string());
            if size.is_empty() { link } else { format!("{link} {}", size.trim()) }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn is_quote(c: &char) -> bool {
    *c == '"' || *c == '\''
}

/// `css` with each `url(...)` swapped where `swap` has a copy, keeping its quotes.
fn swap_urls(css: &str, swap: &impl Fn(&str) -> Option<String>) -> String {
    let mut out = String::with_capacity(css.len());
    let mut rest = css;
    while let Some(at) = rest.find("url(") {
        out.push_str(&rest[..at + 4]);
        rest = &rest[at + 4..];
        let Some(end) = rest.find(')') else { break };
        let inside = rest[..end].trim();
        let quote = inside.chars().next().filter(is_quote);
        match (swap(inside.trim_matches(['"', '\''])), quote) {
            (Some(link), Some(quote)) => out.push_str(&format!("{quote}{link}{quote}")),
            // An unquoted `url()` can't hold these.
            (Some(link), None) if link.contains(['(', ')', '"', '\'']) => out.push_str(&format!("\"{link}\"")),
            (Some(link), None) => out.push_str(&link),
            (None, _) => out.push_str(&rest[..end]),
        }
        rest = &rest[end..];
    }
    out.push_str(rest);
    out
}

/// `css` with each `@import "a.css"` swapped where `swap` has a copy.
fn swap_imports(css: &str, swap: &impl Fn(&str) -> Option<String>) -> String {
    let mut out = String::with_capacity(css.len());
    let mut rest = css;
    while let Some(at) = rest.find("@import") {
        let spaces = rest[at + 7..].len() - rest[at + 7..].trim_start().len();
        out.push_str(&rest[..at + 7 + spaces]);
        rest = &rest[at + 7 + spaces..];
        let Some(quote) = rest.chars().next().filter(is_quote) else { continue };
        let Some(end) = rest[1..].find(quote) else { break };
        let link = &rest[1..=end];
        out.push_str(&format!("{quote}{}{quote}", swap(link).unwrap_or_else(|| link.to_string())));
        rest = &rest[end + 2..];
    }
    out.push_str(rest);
    out
}

/// The page's `<style>` elements rewritten. lol_html hands their text over in pieces: each is held back until the
/// last one, which is replaced by the whole style rewritten.
fn rewrite_styles(html: &str, base: &Url, from: &Path, saved: &Saved) -> String {
    let buffer = RefCell::new(String::new());
    let mut out = Vec::with_capacity(html.len());
    let mut rewriter = HtmlRewriter::new(
        Settings {
            element_content_handlers: vec![text!("style", |chunk| {
                buffer.borrow_mut().push_str(chunk.as_str());
                if chunk.last_in_text_node() {
                    let css = std::mem::take(&mut *buffer.borrow_mut());
                    chunk.replace(&rewrite_css(&css, base, from, saved), ContentType::Html);
                } else {
                    chunk.remove();
                }
                Ok(())
            })],
            ..Settings::new()
        },
        |chunk: &[u8]| out.extend_from_slice(chunk),
    );
    if rewriter.write(html.as_bytes()).is_err() || rewriter.end().is_err() {
        return html.to_string();
    }
    String::from_utf8(out).unwrap_or_else(|_| html.to_string())
}

// ── Public API ────────────────────────────
/// A stylesheet (or a `style`) at `base`, saved at `from`, with its links pointing at the saved copies.
pub fn rewrite_css(css: &str, base: &Url, from: &Path, saved: &Saved) -> String {
    let swap = |link: &str| target(saved, base, from, link);
    swap_imports(&swap_urls(css, &swap), &swap)
}

/// A page from `base`, saved at `from`, with its links pointing at the saved copies. A `<base>` element is dropped,
/// and so is the integrity check of anything rewritten (the copy may differ). The page is saved as UTF-8, so its
/// declared charset says so.
pub fn rewrite_html(html: &str, base: &Url, from: &Path, saved: &Saved) -> String {
    let swap = |link: &str| target(saved, base, from, link);
    let mut out = Vec::with_capacity(html.len());
    let mut rewriter = HtmlRewriter::new(
        Settings {
            element_content_handlers: vec![
                element!("base", |el| {
                    el.remove();
                    Ok(())
                }),
                element!("meta[charset]", |el| {
                    el.set_attribute("charset", "utf-8")?;
                    Ok(())
                }),
                element!("meta[http-equiv]", |el| {
                    if el.get_attribute("http-equiv").is_some_and(|name| name.eq_ignore_ascii_case("content-type")) {
                        el.set_attribute("content", "text/html; charset=utf-8")?;
                    }
                    Ok(())
                }),
                element!("[href], [src], [poster], [data-src], [srcset], [data-srcset], [style]", |el| {
                    let mut changed = false;
                    for name in SINGLE {
                        if let Some(link) = el.get_attribute(name).and_then(|value| swap(&value)) {
                            el.set_attribute(name, &link)?;
                            changed = true;
                        }
                    }
                    for name in SETS {
                        if let Some(value) = el.get_attribute(name) {
                            el.set_attribute(name, &srcset(&value, swap))?;
                        }
                    }
                    if let Some(value) = el.get_attribute("style") {
                        el.set_attribute("style", &rewrite_css(&value, base, from, saved))?;
                    }
                    if changed {
                        el.remove_attribute("integrity");
                    }
                    Ok(())
                }),
            ],
            ..Settings::new()
        },
        |chunk: &[u8]| out.extend_from_slice(chunk),
    );
    if rewriter.write(html.as_bytes()).is_err() || rewriter.end().is_err() {
        return html.to_string();
    }
    let page = String::from_utf8(out).unwrap_or_else(|_| html.to_string());
    rewrite_styles(&page, base, from, saved)
}

#[cfg(test)]
mod tests {
    use super::{Saved, rewrite_css, rewrite_html};
    use reqwest::Url;
    use std::path::{Path, PathBuf};

    fn saved() -> Saved {
        [
            ("https://example.com/", "index.html"),
            ("https://example.com/about", "about/index.html"),
            ("https://example.com/css/site.css", "css/site.css"),
            ("https://example.com/css/base.css", "css/base.css"),
            ("https://example.com/img/a.png", "img/a.png"),
            ("https://example.com/img/a@2x.png", "img/a@2x.png"),
            ("https://example.com/img/bg.png", "img/bg.png"),
            ("https://cdn.net/f.woff2", "_hosts/cdn.net/f.woff2"),
        ]
        .into_iter()
        .map(|(url, local)| (url.to_string(), PathBuf::from(local)))
        .collect()
    }

    #[test]
    fn points_a_pages_links_at_the_copies() {
        let base = Url::parse("https://example.com/about").unwrap();
        let html = r##"<html><head><meta charset="windows-1252"><base href="https://example.com/"><link rel="stylesheet" href="/css/site.css" integrity="sha384-x">
            <style>body { background: url('/img/bg.png') }</style></head><body>
            <a href="/#top">home</a><a href="#here">here</a><a href="https://elsewhere.org/">out</a><a href="/missing">gone</a>
            <img src="/img/a.png" srcset="/img/a.png 1x, /img/a@2x.png 2x, /img/c.png 3x">
            <div style="background:url(/img/bg.png)"></div></body></html>"##;
        let page = rewrite_html(html, &base, Path::new("about/index.html"), &saved());
        assert!(!page.contains("<base") && page.contains(r#"<meta charset="utf-8">"#), "{page}");
        assert!(page.contains(r#"<link rel="stylesheet" href="../css/site.css">"#), "{page}");
        assert!(page.contains("<style>body { background: url('../img/bg.png') }</style>"), "{page}");
        assert!(page.contains(r##"<a href="../index.html#top">"##) && page.contains(r##"<a href="#here">"##));
        assert!(page.contains(r#"href="https://elsewhere.org/""#) && page.contains(r#"href="/missing""#));
        assert!(page.contains(r#"<img src="../img/a.png" srcset="../img/a.png 1x, ../img/a@2x.png 2x, /img/c.png 3x">"#));
        assert!(page.contains(r#"<div style="background:url(../img/bg.png)">"#), "{page}");
    }

    #[test]
    fn points_a_stylesheets_links_at_the_copies() {
        let base = Url::parse("https://example.com/css/site.css").unwrap();
        let css = r#"@import 'base.css'; @import "print.css"; @import url(base.css); .a { background: url(../img/bg.png) }
            @font-face { src: url("https://cdn.net/f.woff2"), url(data:font/woff2;base64,AA) }"#;
        assert_eq!(
            rewrite_css(css, &base, Path::new("css/site.css"), &saved()),
            r#"@import 'base.css'; @import "print.css"; @import url(base.css); .a { background: url(../img/bg.png) }
            @font-face { src: url("../_hosts/cdn.net/f.woff2"), url(data:font/woff2;base64,AA) }"#
        );
    }
}

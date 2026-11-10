//! Search: DuckDuckGo's plain HTML web search, the pages it lists for a query, a page of results at a time with a
//! delay between requests. Its result links are read from the page's markup, which DuckDuckGo may change (ADR-0013).

// ── Imports ───────────────────────────────
use std::cell::RefCell;
use std::time::Duration;

use lol_html::{HtmlRewriter, Settings, element};
use reqwest::header::CONTENT_TYPE;
use reqwest::{Client, Url};

// ── Constants ─────────────────────────────
/// DuckDuckGo's plain HTML search page.
pub const ENGINE: &str = "https://html.duckduckgo.com/html/";
/// Between two requests to DuckDuckGo.
const DELAY: Duration = Duration::from_millis(1500);
/// The most pages of results one search reads.
const MAX_RESULT_PAGES: usize = 10;
const REFUSED: &str = "DuckDuckGo didn't answer the search. Try again in a while.";

// ── Types ─────────────────────────────────
/// What one page of results holds.
#[derive(Debug, Default, PartialEq)]
struct Results {
    links: Vec<String>,
    /// The form fields that ask for the next page.
    next: Option<Vec<(String, String)>>,
}

// ── Private helpers ───────────────────────
/// A result's address: DuckDuckGo's redirect (`//duckduckgo.com/l/?uddg=…`) unwrapped; its own pages and ads dropped.
fn result_link(href: &str) -> Option<String> {
    let href = if href.starts_with("//") { format!("https:{href}") } else { href.to_string() };
    let url = Url::parse(&href).ok()?;
    let host = url.host_str()?;
    if host == "duckduckgo.com" || host.ends_with(".duckduckgo.com") {
        if url.path() != "/l/" {
            return None;
        }
        let target = url.query_pairs().find(|(key, _)| key == "uddg")?.1.into_owned();
        return result_link(&target);
    }
    matches!(url.scheme(), "http" | "https").then(|| url.into())
}

/// Reads a page of results: its links in order, and the form that asks for the next page (the one furthest on).
fn read(html: &str) -> Results {
    let links = RefCell::new(Vec::new());
    let forms: RefCell<Vec<Vec<(String, String)>>> = RefCell::new(Vec::new());
    let mut rewriter = HtmlRewriter::new(
        Settings {
            element_content_handlers: vec![
                element!("a.result__a[href]", |el| {
                    if let Some(link) = el.get_attribute("href").as_deref().map(|href| href.replace("&amp;", "&")).and_then(|href| result_link(&href)) {
                        let mut links = links.borrow_mut();
                        if !links.contains(&link) {
                            links.push(link);
                        }
                    }
                    Ok(())
                }),
                element!("form", |_| {
                    forms.borrow_mut().push(Vec::new());
                    Ok(())
                }),
                element!("form input[type=hidden][name]", |el| {
                    if let Some(form) = forms.borrow_mut().last_mut() {
                        form.push((el.get_attribute("name").unwrap_or_default(), el.get_attribute("value").unwrap_or_default()));
                    }
                    Ok(())
                }),
            ],
            ..Settings::new()
        },
        |_: &[u8]| {},
    );
    let _ = rewriter.write(html.as_bytes());
    let _ = rewriter.end();
    let offset = |form: &Vec<(String, String)>| form.iter().find(|(name, _)| name == "s").and_then(|(_, value)| value.parse::<u32>().ok());
    let next = forms.into_inner().into_iter().filter(|form| offset(form).is_some_and(|s| s > 0)).max_by_key(|form| offset(form));
    Results { links: links.into_inner(), next }
}

/// `pairs` as a form body.
fn form(pairs: &[(String, String)]) -> String {
    let mut url = Url::parse("http://x/").expect("a valid address");
    url.query_pairs_mut().extend_pairs(pairs);
    url.query().unwrap_or_default().to_string()
}

// ── Public API ────────────────────────────
/// Up to `want` result pages for `query` from the search page at `engine`, in its order. Stops early when `stop`
/// says so.
pub async fn search(client: &Client, engine: &str, query: &str, want: usize, stop: &(dyn Fn() -> bool + Sync)) -> Result<Vec<String>, String> {
    let mut first = Url::parse(engine).map_err(|e| e.to_string())?;
    first.query_pairs_mut().append_pair("q", query);
    let failed = |_: reqwest::Error| REFUSED.to_string();
    let mut html = client.get(first).send().await.and_then(|r| r.error_for_status()).map_err(failed)?.text().await.map_err(failed)?;
    let mut found: Vec<String> = Vec::new();
    for page in 0..MAX_RESULT_PAGES {
        let results = read(&html);
        if page == 0 && results.links.is_empty() && results.next.is_none() && !html.contains("No results") {
            return Err(REFUSED.into());
        }
        let before = found.len();
        for link in results.links {
            if found.len() < want && !found.contains(&link) {
                found.push(link);
            }
        }
        let Some(next) = results.next.filter(|_| found.len() < want && found.len() > before && !stop()) else { break };
        tokio::time::sleep(DELAY).await;
        let request = client.post(engine).header(CONTENT_TYPE, "application/x-www-form-urlencoded").body(form(&next));
        html = request.send().await.and_then(|r| r.error_for_status()).map_err(failed)?.text().await.map_err(failed)?;
    }
    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::{read, result_link};

    #[test]
    fn reads_a_page_of_results() {
        let html = r#"<form action="/html/" method="post"><input type="hidden" name="q" value="metin2" /></form>
            <a class="result__a" href="//duckduckgo.com/l/?uddg=https%3A%2F%2Fgameforge.com%2Fplay%2Fmetin2&amp;rut=d8f2">Play</a>
            <a class="result__a" href="https://duckduckgo.com/y.js?ad_domain=shop.gg&amp;u3=x">Ad</a>
            <a class="result__a" href="https://www.metin2vote.com/">Vote</a>
            <a class="result__a" href="https://www.metin2vote.com/">Again</a>
            <form action="/html/" method="post"><input type="hidden" name="q" value="metin2" /><input type="hidden" name="s" value="0" /></form>
            <form action="/html/" method="post"><input type="hidden" name="q" value="metin2" /><input type="hidden" name="s" value="25" /><input type="hidden" name="vqd" value="4-1" /></form>"#;
        let results = read(html);
        assert_eq!(results.links, ["https://gameforge.com/play/metin2", "https://www.metin2vote.com/"]);
        let next = results.next.unwrap();
        assert!(next.contains(&("s".into(), "25".into())) && next.contains(&("vqd".into(), "4-1".into())));
        assert_eq!(read("<p>nothing</p>").next, None);
        assert_eq!(result_link("javascript:alert(1)"), None);
    }
}

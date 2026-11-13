//! Engines: image search engines as Hunt sources (ADR-0025). A reader takes a topic and gives full-size images with
//! the page each is on, read from what the engine's own search page uses: DuckDuckGo's image results as JSON, Bing's
//! result markup, the JSON in Yandex's page. They break when an engine changes its page; each has a test on a sample.

// ── Imports ───────────────────────────────
use std::time::Duration;

use html5ever::tendril::TendrilSink;
use html5ever::{ParseOpts, parse_document};
use markup5ever_rcdom::{NodeData, RcDom};
use regex::Regex;
use reqwest::header::REFERER;
use reqwest::{Client, Url};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

// ── Constants ─────────────────────────────
/// The most requests one engine search makes.
const MAX_REQUESTS: usize = 10;
/// Results Bing gives per request.
const BING_PAGE: usize = 35;

// ── Types ─────────────────────────────────
/// An image search engine Hunt can read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ImageEngine {
    DuckDuckGo,
    Bing,
    Yandex,
}

/// One result: a full-size image, and the page it's on (the image itself when the engine doesn't say).
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Hit {
    pub image: String,
    pub page: String,
}

/// What one engine search asks for.
#[derive(Clone, Copy)]
pub(super) struct Ask<'a> {
    pub query: &'a str,
    /// The most images it gives.
    pub want: usize,
    /// The most requests of results it makes.
    pub requests: usize,
    /// Between two requests.
    pub pause: Duration,
    pub stop: &'a (dyn Fn() -> bool + Sync),
}

/// Where each engine is: the real sites, or local addresses in tests.
#[derive(Debug, Clone)]
pub(super) struct Bases {
    pub duckduckgo: String,
    pub bing: String,
    pub yandex: String,
}

// ── Private helpers ───────────────────────
impl Default for Bases {
    fn default() -> Self {
        Self { duckduckgo: "https://duckduckgo.com".into(), bing: "https://www.bing.com".into(), yandex: "https://yandex.com".into() }
    }
}

/// `text` when it's an http or https link.
fn web_link(text: &str) -> Option<String> {
    Url::parse(text).ok().filter(|url| matches!(url.scheme(), "http" | "https")).map(String::from)
}

fn hit(image: Option<&str>, page: Option<&str>) -> Option<Hit> {
    let image = web_link(image?)?;
    let page = page.and_then(web_link).unwrap_or_else(|| image.clone());
    Some(Hit { image, page })
}

/// Adds `found` to `hits` up to `want`, skipping images already there; returns how many were new.
fn add(hits: &mut Vec<Hit>, found: Vec<Hit>, want: usize) -> usize {
    let before = hits.len();
    for one in found {
        if hits.len() < want && !hits.iter().any(|kept| kept.image == one.image) {
            hits.push(one);
        }
    }
    hits.len() - before
}

/// Every value of the attribute `name` in `html`, in document order, its entities decoded.
fn attributes(html: &str, name: &str) -> Vec<String> {
    let dom = parse_document(RcDom::default(), ParseOpts::default()).one(html);
    // A clone: the document must outlive the walk, since dropping a node empties every node under it.
    let (mut values, mut stack) = (Vec::new(), vec![dom.document.clone()]);
    while let Some(node) = stack.pop() {
        if let NodeData::Element { attrs, .. } = &node.data {
            values.extend(attrs.borrow().iter().filter(|attr| &*attr.name.local == name).map(|attr| attr.value.to_string()));
        }
        stack.extend(node.children.borrow().iter().rev().cloned());
    }
    values
}

/// The token DuckDuckGo's search page carries for its image results.
fn duckduckgo_token(html: &str) -> Option<String> {
    let token = Regex::new(r#"vqd=["']?([0-9-]+)"#).expect("a valid pattern");
    token.captures(html).map(|found| found[1].to_string())
}

/// A page of DuckDuckGo's image results (JSON): its results, and the address of the next page.
fn duckduckgo_results(json: &str) -> (Vec<Hit>, Option<String>) {
    let value: Value = serde_json::from_str(json).unwrap_or_default();
    let hits = value["results"].as_array().into_iter().flatten().filter_map(|result| hit(result["image"].as_str(), result["url"].as_str())).collect();
    (hits, value["next"].as_str().map(String::from))
}

/// A page of Bing's image results: each result's `m` attribute is JSON with the image (`murl`) and its page (`purl`).
fn bing_results(html: &str) -> Vec<Hit> {
    attributes(html, "m")
        .iter()
        .filter_map(|m| serde_json::from_str::<Value>(m).ok())
        .filter_map(|value| hit(value["murl"].as_str(), value["purl"].as_str()))
        .collect()
}

/// Every object in `value` with an `origUrl`: Yandex's results, the page in their `snippet`.
fn yandex_walk(value: &Value, hits: &mut Vec<Hit>) {
    match value {
        Value::Object(object) => {
            if let Some(found) = hit(object.get("origUrl").and_then(Value::as_str), value["snippet"]["url"].as_str()) {
                hits.push(found);
            }
            object.values().for_each(|inner| yandex_walk(inner, hits));
        }
        Value::Array(items) => items.iter().for_each(|inner| yandex_walk(inner, hits)),
        _ => {}
    }
}

/// Yandex's image search page: its results are in the JSON of its `data-state` attributes.
fn yandex_results(html: &str) -> Vec<Hit> {
    let mut hits = Vec::new();
    for state in attributes(html, "data-state") {
        if let Ok(value) = serde_json::from_str::<Value>(&state) {
            yandex_walk(&value, &mut hits);
        }
    }
    hits
}

/// Reads `url`; its final address (after redirects) and its text, or `failed`.
async fn get(client: &Client, url: Url, referer: Option<&str>, failed: &str) -> Result<(Url, String), String> {
    let mut request = client.get(url);
    if let Some(referer) = referer {
        request = request.header(REFERER, referer);
    }
    let response = request.send().await.and_then(|r| r.error_for_status()).map_err(|_| failed.to_string())?;
    let address = response.url().clone();
    Ok((address, response.text().await.map_err(|_| failed.to_string())?))
}

/// `base` with `path` and `pairs`.
fn address(base: &str, path: &str, pairs: &[(&str, &str)]) -> Result<Url, String> {
    let mut url = Url::parse(base).and_then(|base| base.join(path)).map_err(|e| e.to_string())?;
    url.query_pairs_mut().extend_pairs(pairs);
    Ok(url)
}

async fn duckduckgo(client: &Client, base: &str, ask: &Ask<'_>) -> Result<Vec<Hit>, String> {
    const FAILED: &str = "DuckDuckGo didn't answer the image search. Try again in a while.";
    let Ask { query, want, requests, pause, stop } = *ask;
    let (_, html) = get(client, address(base, "/", &[("q", query), ("iax", "images"), ("ia", "images")])?, None, FAILED).await?;
    let token = duckduckgo_token(&html).ok_or(FAILED)?;
    let referer = format!("{base}/");
    let mut next = address(base, "/i.js", &[("l", "wt-wt"), ("o", "json"), ("q", query), ("vqd", &token), ("f", ",,,,,"), ("p", "1")])?;
    let mut hits = Vec::new();
    for request in 0..requests {
        tokio::time::sleep(pause).await;
        let json = match get(client, next.clone(), Some(&referer), FAILED).await {
            Ok((_, json)) => json,
            Err(problem) if request == 0 => return Err(problem),
            Err(_) => break,
        };
        let (found, more) = duckduckgo_results(&json);
        let added = add(&mut hits, found, want);
        // The next page's address leaves out the token.
        let Some(more) = more.filter(|_| added > 0 && hits.len() < want && !stop()) else { break };
        next = Url::parse(base).and_then(|base| base.join(&format!("/{more}"))).map_err(|e| e.to_string())?;
        next.query_pairs_mut().append_pair("vqd", &token);
    }
    Ok(hits)
}

async fn bing(client: &Client, base: &str, ask: &Ask<'_>) -> Result<Vec<Hit>, String> {
    const FAILED: &str = "Bing didn't answer the image search. Try again in a while.";
    let Ask { query, want, requests, pause, stop } = *ask;
    let mut hits = Vec::new();
    for request in 0..requests {
        if request > 0 {
            tokio::time::sleep(pause).await;
        }
        let first = (request * BING_PAGE).to_string();
        let count = BING_PAGE.to_string();
        let url = address(base, "/images/async", &[("q", query), ("first", &first), ("count", &count), ("mmasync", "1")])?;
        let html = match get(client, url, None, FAILED).await {
            Ok((_, html)) => html,
            Err(problem) if request == 0 => return Err(problem),
            Err(_) => break,
        };
        if add(&mut hits, bing_results(&html), want) == 0 || hits.len() >= want || stop() {
            break;
        }
    }
    Ok(hits)
}

/// One page of results: Yandex asks for a CAPTCHA on any later one.
async fn yandex(client: &Client, base: &str, ask: &Ask<'_>) -> Result<Vec<Hit>, String> {
    const FAILED: &str = "Yandex didn't answer the image search. Try again in a while.";
    let (landed, html) = get(client, address(base, "/images/search", &[("text", ask.query)])?, None, FAILED).await?;
    if landed.path().contains("captcha") {
        return Err("Yandex asked for a CAPTCHA. Try again in a while.".into());
    }
    let mut hits = Vec::new();
    add(&mut hits, yandex_results(&html), ask.want);
    Ok(hits)
}

// ── Public API ────────────────────────────
impl ImageEngine {
    /// The source's name in Hunt.
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::DuckDuckGo => "DuckDuckGo Images",
            Self::Bing => "Bing Images",
            Self::Yandex => "Yandex Images",
        }
    }

    /// The turn it takes to search: sources asking the same host share one (0 is DuckDuckGo's, which the web search
    /// uses too).
    pub(super) fn turn(self) -> usize {
        match self {
            Self::DuckDuckGo => 0,
            Self::Bing => 1,
            Self::Yandex => 2,
        }
    }
}

/// Images from `engine` as `ask` says, stopping early when its `stop` says so. A request that fails after the first
/// ends the search with what it found.
pub(super) async fn search(client: &Client, bases: &Bases, engine: ImageEngine, ask: &Ask<'_>) -> Result<Vec<Hit>, String> {
    let ask = Ask { requests: ask.requests.clamp(1, MAX_REQUESTS), ..*ask };
    match engine {
        ImageEngine::DuckDuckGo => duckduckgo(client, &bases.duckduckgo, &ask).await,
        ImageEngine::Bing => bing(client, &bases.bing, &ask).await,
        ImageEngine::Yandex => yandex(client, &bases.yandex, &ask).await,
    }
}

#[cfg(test)]
mod tests {
    use super::{Ask, Bases, Hit, ImageEngine, bing_results, duckduckgo_results, duckduckgo_token, search, yandex_results};
    use std::time::Duration;

    fn hit(image: &str, page: &str) -> Hit {
        Hit { image: image.into(), page: page.into() }
    }

    #[test]
    fn reads_duckduckgo() {
        assert_eq!(duckduckgo_token(r#"<script>DDG.deep.initialize('/d.js?q=x&vqd="4-1234567890"&p=1');</script>"#).as_deref(), Some("4-1234567890"));
        assert_eq!(duckduckgo_token("<p>nothing</p>"), None);
        let json = r#"{"results": [
            {"image": "https://a.com/full.jpg", "url": "https://a.com/page", "thumbnail": "https://t.com/1"},
            {"image": "javascript:alert(1)", "url": "https://b.com/"},
            {"image": "https://c.com/full.png"}
        ], "next": "i.js?q=cats&o=json&p=1&s=100"}"#;
        let (hits, next) = duckduckgo_results(json);
        assert_eq!(hits, [hit("https://a.com/full.jpg", "https://a.com/page"), hit("https://c.com/full.png", "https://c.com/full.png")]);
        assert_eq!(next.as_deref(), Some("i.js?q=cats&o=json&p=1&s=100"));
        assert_eq!(duckduckgo_results("not json"), (Vec::new(), None));
    }

    #[test]
    fn reads_bing() {
        let html = r#"<div class="imgpt"><a class="iusc" style="height:180px" m="{&quot;cid&quot;:&quot;x&quot;,&quot;purl&quot;:&quot;https://a.com/cats/&quot;,&quot;murl&quot;:&quot;https://a.com/cat.jpg?w=1&amp;h=2&quot;,&quot;turl&quot;:&quot;https://tse.mm.bing.net/th?id=1&quot;}" href="/images/search?view=detailV2"></a></div>
            <a class="iusc" m="not json"></a><div m="{&quot;murl&quot;:&quot;ftp://b.com/x.jpg&quot;}"></div>"#;
        assert_eq!(bing_results(html), [hit("https://a.com/cat.jpg?w=1&h=2", "https://a.com/cats/")]);
    }

    #[test]
    fn reads_yandex() {
        let state = r#"{"initialState":{"serpList":{"items":{"entities":{"a1":{"alt":"Cat","origUrl":"https://a.com/cat.jpg","snippet":{"url":"https://a.com/wiki/Cat","domain":"a.com"}},"b2":{"origUrl":"https://b.com/dog.png"}}}},"other":[{"origUrl":"data:image/png;base64,AAAA"}]}}"#;
        let html = format!(r#"<div class="Root" data-state="{}"></div><div data-state="{{&quot;form&quot;:{{}}}}"></div>"#, state.replace('"', "&quot;"));
        assert_eq!(yandex_results(&html), [hit("https://a.com/cat.jpg", "https://a.com/wiki/Cat"), hit("https://b.com/dog.png", "https://b.com/dog.png")]);
    }

    /// Proves the readers still match the engines: run with `cargo test -p scrappy-core -- --ignored` when one fails.
    #[tokio::test]
    #[ignore = "searches DuckDuckGo, Bing and Yandex for images"]
    async fn reads_the_real_engines() {
        let client = reqwest::Client::builder().user_agent(crate::app::USER_AGENT).build().unwrap();
        let stop = || false;
        let ask = Ask { query: "red panda", want: 60, requests: 2, pause: Duration::from_millis(1500), stop: &stop };
        for engine in [ImageEngine::DuckDuckGo, ImageEngine::Bing, ImageEngine::Yandex] {
            let hits = search(&client, &Bases::default(), engine, &ask).await.unwrap_or_else(|e| panic!("{}: {e}", engine.name()));
            eprintln!("{}: {} images, first {:?}", engine.name(), hits.len(), hits.first());
            assert!(hits.len() >= 20, "{}: only {} images", engine.name(), hits.len());
        }
    }
}

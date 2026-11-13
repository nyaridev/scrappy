//! Message: what the extension may send the app and what the app answers, and the checks every request passes
//! before anything uses it (ADR-0005, ADR-0006).

// ── Imports ───────────────────────────────
use reqwest::Url;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::grab::{self, MAX_FINDS, PageScan, Seen};
use crate::vault::{self, Cookie};

// ── Constants ─────────────────────────────
const MAX_URL: usize = 8 * 1024;
const MAX_TITLE: usize = 500;
const MAX_HEADERS: usize = 16;
const MAX_HEADER_VALUE: usize = 1024;
const MAX_MIME: usize = 200;
/// The only request headers passed on. Cookies and credentials never travel this way (ADR-0005).
const ALLOWED_HEADERS: &[&str] = &["Referer", "Origin", "User-Agent"];

// ── Types ─────────────────────────────────
/// What the extension found the media as.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../extension/lib/types/")]
pub enum MediaKind {
    Video,
    Image,
}

/// A request header the page's request carried.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../extension/lib/types/")]
pub struct Header {
    pub name: String,
    pub value: String,
}

/// One site's cookies the user agreed to share: for this download only, or remembered in the vault.
#[derive(Debug, Clone, PartialEq, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../extension/lib/types/")]
pub struct CookieGrant {
    pub remember: bool,
    pub cookies: Vec<Cookie>,
}

/// A request or link the extension noted on a page.
#[derive(Debug, Clone, PartialEq, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../extension/lib/types/")]
pub struct Noted {
    pub url: String,
    /// The response's `Content-Type`, when the request was seen.
    pub mime: Option<String>,
    #[ts(type = "number | null")]
    pub size: Option<u64>,
}

/// A message from the extension.
#[derive(Debug, Clone, PartialEq, Deserialize, TS)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
#[ts(export, export_to = "../../../extension/lib/types/")]
pub enum Request {
    /// Is the app there?
    Hello,
    /// Download this. `url` is the media itself, or the page for sites yt-dlp knows.
    Send {
        url: String,
        kind: MediaKind,
        page_url: Option<String>,
        title: Option<String>,
        #[serde(default)]
        headers: Vec<Header>,
        /// The link's site's cookies, sent only after the app asked for them with `NeedsCookies`.
        #[serde(default)]
        cookies: Option<CookieGrant>,
        /// A preset's id, from a button that names one (MP4, MP3); none for the start preset.
        #[serde(default)]
        preset: Option<String>,
    },
    /// Open this page in Grab with everything the extension noted on it.
    Grab {
        page_url: String,
        title: Option<String>,
        finds: Vec<Noted>,
    },
    /// What would downloading this page save? Only a site reader's answer counts, not yt-dlp's generic one
    /// (ADR-0042).
    Look { url: String },
    /// The page's site, and whether its sign-in is remembered.
    Site { url: String },
    /// Remember this page's site's sign-in, downloading nothing.
    SignIn { url: String, cookies: Vec<Cookie> },
}

/// The app's answer to one request.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
#[ts(export, export_to = "../../../extension/lib/types/")]
pub enum Reply {
    Hello { version: String },
    /// Queued as the download with this id.
    Queued { id: u32 },
    /// The site wants a signed-in visitor: ask the user, then send again with that one site's cookies.
    NeedsCookies { site: String },
    /// Opened in Grab with this many finds.
    Opened { count: u32 },
    /// A page a site reader knows: what downloading it saves. `count` for a list, post or profile; `files` when its
    /// files are saved as they are.
    Video {
        title: String,
        channel: Option<String>,
        duration: Option<f64>,
        /// A `data:` URL.
        thumbnail: Option<String>,
        count: Option<u32>,
        files: bool,
    },
    /// No site reader knows the page.
    NotVideo,
    /// The page's site; `signed_in` when its sign-in is remembered.
    Site { site: String, signed_in: bool },
    /// The site's sign-in is remembered.
    SignedIn { site: String },
    Error { message: String },
}

/// A send that passed every check.
#[derive(Debug, Clone, PartialEq)]
pub struct Checked {
    pub url: String,
    pub kind: MediaKind,
    pub page_url: Option<String>,
    pub title: Option<String>,
    /// Only the allowed headers, named as `ALLOWED_HEADERS` spells them.
    pub headers: Vec<(String, String)>,
    pub grant: Option<Grant>,
}

/// A cookie grant that passed its checks: every cookie belongs to the link's own site.
#[derive(Debug, Clone, PartialEq)]
pub struct Grant {
    pub site: String,
    pub remember: bool,
    pub cookies: Vec<Cookie>,
}

// ── Private helpers ───────────────────────
fn web_link(input: &str) -> Result<String, String> {
    if input.len() > MAX_URL {
        return Err("The link is too long.".into());
    }
    let url = Url::parse(input.trim()).map_err(|_| "Not a link.".to_string())?;
    match url.scheme() {
        "http" | "https" => Ok(url.into()),
        _ => Err("Only http and https links work.".into()),
    }
}

/// The allowed headers, with their canonical names; the rest are dropped.
fn allowed(headers: Vec<Header>) -> Result<Vec<(String, String)>, String> {
    if headers.len() > MAX_HEADERS {
        return Err("Too many headers.".into());
    }
    let mut kept = Vec::new();
    for header in headers {
        let Some(name) = ALLOWED_HEADERS.iter().find(|name| name.eq_ignore_ascii_case(header.name.trim())) else {
            continue;
        };
        let value = header.value.trim();
        if value.len() > MAX_HEADER_VALUE || value.chars().any(char::is_control) {
            return Err(format!("The {name} header can't be used."));
        }
        if !kept.iter().any(|(kept_name, _)| kept_name == name) {
            kept.push(((*name).to_string(), value.to_string()));
        }
    }
    Ok(kept)
}

/// A title without control characters, at most 500 characters, or `None` when nothing's left.
fn clean_title(title: Option<&str>) -> Option<String> {
    title
        .map(|title| title.chars().filter(|c| !c.is_control()).take(MAX_TITLE).collect::<String>())
        .map(|title| title.trim().to_string())
        .filter(|title| !title.is_empty())
}

// ── Public API ────────────────────────────
impl Request {
    /// Reads one frame's JSON.
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        serde_json::from_slice(bytes).map_err(|_| "The app couldn't read the message.".into())
    }
}

/// Checks a send's fields: web links only, a short title without control characters, allowed headers only, and
/// cookies only for the link's own site (ADR-0005).
pub fn check(
    url: &str,
    kind: MediaKind,
    page_url: Option<&str>,
    title: Option<&str>,
    headers: Vec<Header>,
    cookies: Option<CookieGrant>,
) -> Result<Checked, String> {
    let url = web_link(url)?;
    let grant = match cookies {
        Some(CookieGrant { remember, cookies }) => {
            let site = vault::site_of(&url).ok_or("This link has no site to sign in to.")?;
            let cookies = vault::check_cookies(&site, cookies)?;
            Some(Grant { site, remember, cookies })
        }
        None => None,
    };
    let title = clean_title(title);
    Ok(Checked { url, kind, page_url: page_url.map(web_link).transpose()?, title, headers: allowed(headers)?, grant })
}

/// Checks a sign-in shared without a download: a web link with a site, and cookies of that site only (ADR-0005).
pub fn check_sign_in(url: &str, cookies: Vec<Cookie>) -> Result<(String, Vec<Cookie>), String> {
    let site = vault::site_of(&web_link(url)?).ok_or("This page has no site to sign in to.")?;
    if cookies.is_empty() {
        return Err(format!("No cookies for {site}. Sign in there in your browser first."));
    }
    let cookies = vault::check_cookies(&site, cookies)?;
    Ok((site, cookies))
}

/// Checks a link the extension asks about: a web link only.
pub fn check_page(url: &str) -> Result<String, String> {
    web_link(url)
}

/// Checks a page handed to Grab and sorts what was noted on it: a web link, a clean title, at most 2000 notes.
/// Notes too long to be real are dropped; `grab` drops anything that isn't a web link or a find.
pub fn check_grab(page_url: &str, title: Option<&str>, finds: Vec<Noted>) -> Result<PageScan, String> {
    let url = web_link(page_url)?;
    if finds.len() > MAX_FINDS {
        return Err("The page has too many things to grab.".into());
    }
    let seen = finds
        .into_iter()
        .filter(|noted| noted.url.len() <= MAX_URL && noted.mime.as_ref().is_none_or(|mime| mime.len() <= MAX_MIME))
        .map(|noted| Seen { url: noted.url, mime: noted.mime, size: noted.size });
    Ok(PageScan { url, title: clean_title(title), finds: grab::collect(seen), pages: 1, kept: Default::default() })
}

impl Reply {
    /// The reply as one frame's JSON.
    pub fn to_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap_or_else(|_| br#"{"type":"error","message":"Reply failed."}"#.to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::{CookieGrant, Header, MediaKind, Noted, Reply, Request, check, check_grab, check_sign_in};
    use crate::vault::Cookie;

    fn cookie(domain: &str) -> Cookie {
        Cookie {
            domain: domain.into(),
            name: "SID".into(),
            value: "v".into(),
            path: "/".into(),
            secure: true,
            http_only: true,
            host_only: false,
            expires: None,
        }
    }

    fn header(name: &str, value: &str) -> Header {
        Header { name: name.into(), value: value.into() }
    }

    #[test]
    fn reads_the_extensions_messages() {
        assert_eq!(Request::parse(br#"{"type":"hello"}"#).unwrap(), Request::Hello);
        let send = Request::parse(
            br#"{"type":"send","url":"https://a.com/v.mp4","kind":"video","pageUrl":"https://a.com/","title":"A",
                "headers":[{"name":"Referer","value":"https://a.com/"}]}"#,
        )
        .unwrap();
        assert!(matches!(send, Request::Send { kind: MediaKind::Video, .. }));
        assert!(Request::parse(br#"{"type":"delete","path":"C:/"}"#).is_err());
        assert!(Request::parse(b"not json").is_err());
    }

    #[test]
    fn keeps_only_safe_parts() {
        let headers = vec![
            header("cookie", "session=secret"),
            header("Authorization", "Bearer x"),
            header("referer", "https://a.com/page"),
            header("Referer", "https://second.com/"),
            header("user-agent", "Mozilla/5.0"),
        ];
        let page = Some("https://a.com/page");
        let checked = check("https://a.com/v.m3u8", MediaKind::Video, page, Some(" A\u{7}B "), headers, None).unwrap();
        assert_eq!(
            checked.headers,
            [("Referer".into(), "https://a.com/page".into()), ("User-Agent".into(), "Mozilla/5.0".into())]
        );
        assert_eq!(checked.title.as_deref(), Some("AB"));
    }

    #[test]
    fn refuses_what_isnt_a_web_link_or_a_clean_header() {
        let none = || Vec::new();
        assert!(check("file:///C:/secret", MediaKind::Image, None, None, none(), None).is_err());
        assert!(check("https://a.com/i.png", MediaKind::Image, Some("javascript:alert(1)"), None, none(), None).is_err());
        assert!(check(&format!("https://a.com/{}", "a".repeat(9000)), MediaKind::Image, None, None, none(), None).is_err());
        let injected = vec![header("Referer", "https://a.com/\r\nCookie: x")];
        assert!(check("https://a.com/i.png", MediaKind::Image, None, None, injected, None).is_err());
        let many = (0..17).map(|n| header("X-Any", &n.to_string())).collect();
        assert!(check("https://a.com/i.png", MediaKind::Image, None, None, many, None).is_err());
    }

    #[test]
    fn takes_cookies_only_for_the_links_own_site() {
        let grant = |domain: &str| Some(CookieGrant { remember: true, cookies: vec![cookie(domain)] });
        let url = "https://www.youtube.com/watch?v=a";
        let checked = check(url, MediaKind::Video, None, None, Vec::new(), grant(".youtube.com")).unwrap();
        assert_eq!(checked.grant.unwrap().site, "youtube.com");
        assert!(check(url, MediaKind::Video, None, None, Vec::new(), grant(".google.com")).is_err());
        let short = check("https://youtu.be/a", MediaKind::Video, None, None, Vec::new(), grant(".youtube.com"));
        assert_eq!(short.unwrap().grant.unwrap().site, "youtube.com");
    }

    #[test]
    fn shares_a_sign_in_only_with_its_own_site() {
        let page = "https://www.instagram.com/p/abc/";
        let (site, kept) = check_sign_in(page, vec![cookie(".instagram.com")]).unwrap();
        assert_eq!((site.as_str(), kept.len()), ("instagram.com", 1));
        assert!(check_sign_in(page, vec![cookie(".evil.com")]).is_err());
        assert!(check_sign_in(page, Vec::new()).is_err());
        assert!(check_sign_in("file:///C:/x", vec![cookie(".instagram.com")]).is_err());
        let mp3 = br#"{"type":"send","url":"https://a.com/v","kind":"video","pageUrl":null,"title":null,"preset":"mp3"}"#;
        assert!(matches!(Request::parse(mp3).unwrap(), Request::Send { preset: Some(id), .. } if id == "mp3"));
        let ask = Request::parse(br#"{"type":"look","url":"https://a.com/"}"#).unwrap();
        assert_eq!(ask, Request::Look { url: "https://a.com/".into() });
    }

    #[test]
    fn sorts_a_page_handed_to_grab() {
        let noted = |url: &str, mime: Option<&str>| Noted { url: url.into(), mime: mime.map(Into::into), size: None };
        let finds = vec![
            noted("https://a.com/live", Some("application/vnd.apple.mpegurl")),
            noted("https://a.com/app.js", Some("text/javascript")),
            noted(&format!("https://a.com/{}.jpg", "a".repeat(9000)), None),
            noted("javascript:alert(1)", None),
        ];
        let page = check_grab("https://a.com/", Some(" Live\u{7} "), finds).unwrap();
        assert_eq!((page.title.as_deref(), page.finds.len()), (Some("Live"), 1));
        assert!(check_grab("file:///C:/", None, Vec::new()).is_err());
        let many = (0..2001).map(|n| noted(&format!("https://a.com/{n}.jpg"), None)).collect();
        assert!(check_grab("https://a.com/", None, many).is_err());
        let grab = Request::parse(br#"{"type":"grab","pageUrl":"https://a.com/","title":null,"finds":[]}"#);
        assert!(matches!(grab, Ok(Request::Grab { .. })));
    }

    #[test]
    fn writes_replies_the_extension_reads() {
        let bytes = Reply::Queued { id: 7 }.to_bytes();
        assert_eq!(String::from_utf8(bytes).unwrap(), r#"{"type":"queued","id":7}"#);
        let bytes = Reply::Error { message: "No".into() }.to_bytes();
        assert_eq!(String::from_utf8(bytes).unwrap(), r#"{"type":"error","message":"No"}"#);
        let bytes = Reply::NeedsCookies { site: "x.com".into() }.to_bytes();
        assert_eq!(String::from_utf8(bytes).unwrap(), r#"{"type":"needsCookies","site":"x.com"}"#);
    }
}

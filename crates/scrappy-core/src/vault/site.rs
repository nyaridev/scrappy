//! Site: the site a link belongs to (`www.youtube.com/watch` → `youtube.com`), which is what cookies are kept and
//! asked for by, and whether a cookie belongs to it.

// ── Imports ───────────────────────────────
use reqwest::Url;

// ── Constants ─────────────────────────────
/// Short or old domains that sign in on another one.
const ALIASES: &[(&str, &str)] = &[
    ("youtu.be", "youtube.com"),
    ("youtube-nocookie.com", "youtube.com"),
    ("twitter.com", "x.com"),
    ("fb.watch", "facebook.com"),
    ("fb.com", "facebook.com"),
    ("instagr.am", "instagram.com"),
];

// ── Public API ────────────────────────────
/// The site of a link: its registrable domain, through the aliases. `None` for something that isn't a web link.
pub fn site_of(url: &str) -> Option<String> {
    let url = Url::parse(url).ok()?;
    if !matches!(url.scheme(), "http" | "https") {
        return None;
    }
    let host = url.host_str()?.trim_end_matches('.').to_lowercase();
    if host.contains(':') || host.parse::<std::net::Ipv4Addr>().is_ok() || !host.contains('.') {
        return Some(host);
    }
    let labels: Vec<&str> = host.split('.').collect();
    let count = labels.len();
    // shortcut: guesses two-part suffixes (co.uk, com.au) by length; use the public suffix list if sites look wrong.
    let keep = if count >= 3 && labels[count - 2].len() <= 3 && labels[count - 1].len() == 2 { 3 } else { 2 };
    let site = labels[count - keep..].join(".");
    Some(ALIASES.iter().find(|(from, _)| *from == site).map_or(site, |(_, to)| (*to).to_string()))
}

/// Whether a cookie's domain (`.youtube.com`, `accounts.youtube.com`) belongs to `site`.
pub fn belongs_to(cookie_domain: &str, site: &str) -> bool {
    let domain = cookie_domain.trim_start_matches('.').to_lowercase();
    domain == site || domain.ends_with(&format!(".{site}"))
}

#[cfg(test)]
mod tests {
    use super::{belongs_to, site_of};

    #[test]
    fn names_the_site_of_a_link() {
        assert_eq!(site_of("https://www.youtube.com/watch?v=a").as_deref(), Some("youtube.com"));
        assert_eq!(site_of("https://youtu.be/a").as_deref(), Some("youtube.com"));
        assert_eq!(site_of("https://twitter.com/a/status/1").as_deref(), Some("x.com"));
        assert_eq!(site_of("https://news.bbc.co.uk/x").as_deref(), Some("bbc.co.uk"));
        assert_eq!(site_of("http://localhost:3000/a").as_deref(), Some("localhost"));
        assert_eq!(site_of("file:///C:/a"), None);
        assert_eq!(site_of("not a link"), None);
    }

    #[test]
    fn matches_cookies_to_their_site() {
        assert!(belongs_to(".youtube.com", "youtube.com"));
        assert!(belongs_to("accounts.youtube.com", "youtube.com"));
        assert!(belongs_to("youtube.com", "youtube.com"));
        assert!(!belongs_to(".notyoutube.com", "youtube.com"));
        assert!(!belongs_to(".google.com", "youtube.com"));
    }
}

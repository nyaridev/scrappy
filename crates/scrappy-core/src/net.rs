//! Net: the proxy every request goes through when the user set one (ADR-0034), and the HTTP clients that use it, named
//! with Scrappy's User-Agent. The tools get the proxy from `media::engine` and Grab's browser from `grab::scan`.

// ── Imports ───────────────────────────────
use std::sync::{PoisonError, RwLock};

use reqwest::{ClientBuilder, Proxy, Url};

use crate::app::USER_AGENT;

// ── Constants ─────────────────────────────
const SCHEMES: &[&str] = &["http", "https", "socks5", "socks5h"];

// ── State ─────────────────────────────────
static PROXY: RwLock<Option<String>> = RwLock::new(None);

// ── Public API ────────────────────────────
/// `text` as a proxy address (`None` when empty), or why it isn't one. A user or password is refused: they'd be
/// secrets in the plain settings file (ADR-0005).
pub fn check_proxy(text: &str) -> Result<Option<String>, String> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(None);
    }
    let url = Url::parse(text).map_err(|_| "The proxy is an address like socks5://127.0.0.1:9050.".to_string())?;
    if !SCHEMES.contains(&url.scheme()) || url.host_str().is_none() {
        return Err("The proxy starts with http://, https://, socks5:// or socks5h://.".into());
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err("A proxy with a user or password isn't supported.".into());
    }
    Ok(Some(text.trim_end_matches('/').to_string()))
}

/// Sends every request after this through `proxy` (checked), or straight out with `None`.
pub fn set_proxy(proxy: Option<String>) {
    *PROXY.write().unwrap_or_else(PoisonError::into_inner) = proxy;
}

/// The proxy requests go through, if any.
pub fn proxy() -> Option<String> {
    PROXY.read().unwrap_or_else(PoisonError::into_inner).clone()
}

/// An HTTP client builder with Scrappy's User-Agent and the proxy.
pub fn client() -> ClientBuilder {
    let builder = reqwest::Client::builder().user_agent(USER_AGENT);
    match proxy().map(|proxy| Proxy::all(&proxy)) {
        Some(Ok(proxy)) => builder.proxy(proxy),
        _ => builder,
    }
}

#[cfg(test)]
mod tests {
    use super::check_proxy;

    #[test]
    fn checks_a_proxy() {
        assert_eq!(check_proxy("  ").unwrap(), None);
        assert_eq!(check_proxy("socks5h://127.0.0.1:9050/").unwrap().as_deref(), Some("socks5h://127.0.0.1:9050"));
        assert!(check_proxy("ftp://host:21").unwrap_err().contains("starts with"));
        assert!(check_proxy("http://me:secret@host:8080").unwrap_err().contains("password"));
        assert!(check_proxy("not a proxy").is_err());
    }
}

//! Cookie: one browser cookie as the extension sends it, the checks it passes, and the temporary Netscape cookies
//! file a tool reads it from. The file is owner-only and deleted when the run ends, however it ends (ADR-0005).

// ── Imports ───────────────────────────────
use std::fmt;
use std::fs;
use std::io::Write;

use serde::{Deserialize, Serialize};
use tempfile::NamedTempFile;
use ts_rs::TS;

use super::site::belongs_to;

// ── Constants ─────────────────────────────
const MAX_COOKIES: usize = 1000;
const MAX_NAME: usize = 256;
const MAX_VALUE: usize = 4096;
const MAX_DOMAIN: usize = 253;
const MAX_PATH: usize = 1024;
/// Every cookies file starts with this, so files a crash left behind can be found and removed.
const FILE_PREFIX: &str = "scrappy-cookies-";

// ── Types ─────────────────────────────────
/// A browser cookie. Its `Debug` hides the value, so a cookie never reaches a log by accident.
#[derive(Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../extension/lib/types/")]
pub struct Cookie {
    pub domain: String,
    pub name: String,
    pub value: String,
    pub path: String,
    pub secure: bool,
    pub http_only: bool,
    /// Sent only to `domain` itself, not its subdomains.
    pub host_only: bool,
    /// Unix seconds; `None` for a session cookie.
    pub expires: Option<f64>,
}

/// A temporary cookies file; deleted when dropped.
pub struct CookieFile(NamedTempFile);

// ── Private helpers ───────────────────────
fn clean(field: &str, max: usize) -> bool {
    field.len() <= max && !field.chars().any(char::is_control)
}

/// One line of a Netscape cookies file.
fn line(cookie: &Cookie) -> String {
    let domain = cookie.domain.trim_start_matches('.');
    let (domain, subdomains) = if cookie.host_only { (domain.to_string(), "FALSE") } else { (format!(".{domain}"), "TRUE") };
    let prefix = if cookie.http_only { "#HttpOnly_" } else { "" };
    let path = if cookie.path.starts_with('/') { cookie.path.as_str() } else { "/" };
    let secure = if cookie.secure { "TRUE" } else { "FALSE" };
    let expires = cookie.expires.map_or(0, |seconds| seconds.max(0.0) as u64);
    format!("{prefix}{domain}\t{subdomains}\t{path}\t{secure}\t{expires}\t{}\t{}\n", cookie.name, cookie.value)
}

// ── Public API ────────────────────────────
impl fmt::Debug for Cookie {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Cookie").field("domain", &self.domain).field("name", &self.name).finish_non_exhaustive()
    }
}

/// Checks cookies the extension sent for `site`: each belongs to it, and no field could break the cookies file.
pub fn check_cookies(site: &str, cookies: Vec<Cookie>) -> Result<Vec<Cookie>, String> {
    if cookies.is_empty() {
        return Err(format!("No cookies for {site}. Sign in there in your browser first."));
    }
    if cookies.len() > MAX_COOKIES {
        return Err("Too many cookies.".into());
    }
    for cookie in &cookies {
        if !belongs_to(&cookie.domain, site) {
            return Err(format!("A cookie isn't for {site}."));
        }
        let fields_ok = clean(&cookie.domain, MAX_DOMAIN)
            && clean(&cookie.name, MAX_NAME)
            && clean(&cookie.value, MAX_VALUE)
            && clean(&cookie.path, MAX_PATH)
            && cookie.expires.is_none_or(f64::is_finite);
        if !fields_ok {
            return Err("A cookie can't be used.".into());
        }
    }
    Ok(cookies)
}

/// The cookies as a Netscape cookies file, the format yt-dlp and gallery-dl read.
pub fn netscape(cookies: &[Cookie]) -> String {
    let mut text = String::from("# Netscape HTTP Cookie File\n");
    for cookie in cookies {
        text.push_str(&line(cookie));
    }
    text
}

impl CookieFile {
    /// Writes `cookies` to a new temporary file, readable only by this user.
    pub fn write(cookies: &[Cookie]) -> Result<Self, String> {
        let error = |e: std::io::Error| format!("Can't prepare the sign-in for the download: {e}");
        // tempfile creates it owner-only (0600) on Unix; on Windows the user's temp folder is private to them.
        let mut file = tempfile::Builder::new().prefix(FILE_PREFIX).suffix(".txt").tempfile().map_err(error)?;
        file.write_all(netscape(cookies).as_bytes()).map_err(error)?;
        file.flush().map_err(error)?;
        Ok(Self(file))
    }

    /// Where the file is, for the tool's `--cookies`.
    pub fn path(&self) -> &std::path::Path {
        self.0.path()
    }
}

/// Removes cookies files an earlier run left behind (after a crash). Called once at start-up.
pub fn remove_leftover_files() {
    let Ok(entries) = fs::read_dir(std::env::temp_dir()) else { return };
    for entry in entries.filter_map(Result::ok) {
        if entry.file_name().to_string_lossy().starts_with(FILE_PREFIX) {
            let _ = fs::remove_file(entry.path());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Cookie, CookieFile, check_cookies, netscape};

    pub(crate) fn cookie(domain: &str, name: &str, value: &str) -> Cookie {
        Cookie {
            domain: domain.into(),
            name: name.into(),
            value: value.into(),
            path: "/".into(),
            secure: true,
            http_only: false,
            host_only: false,
            expires: Some(1_900_000_000.0),
        }
    }

    #[test]
    fn takes_only_the_sites_clean_cookies() {
        assert!(check_cookies("youtube.com", vec![cookie(".youtube.com", "SID", "a")]).is_ok());
        assert!(check_cookies("youtube.com", vec![cookie(".google.com", "SID", "a")]).unwrap_err().contains("isn't for"));
        assert!(check_cookies("youtube.com", vec![cookie(".youtube.com", "SID", "a\tb")]).is_err());
        assert!(check_cookies("youtube.com", vec![cookie(".youtube.com", "SID", "a\nb")]).is_err());
        assert!(check_cookies("youtube.com", Vec::new()).unwrap_err().contains("Sign in"));
    }

    #[test]
    fn writes_netscape_lines() {
        let mut host_only = cookie("www.youtube.com", "PREF", "f6=40000000");
        (host_only.host_only, host_only.http_only, host_only.expires) = (true, true, None);
        let text = netscape(&[cookie(".youtube.com", "SID", "abc"), host_only]);
        assert_eq!(
            text,
            "# Netscape HTTP Cookie File\n\
             .youtube.com\tTRUE\t/\tTRUE\t1900000000\tSID\tabc\n\
             #HttpOnly_www.youtube.com\tFALSE\t/\tTRUE\t0\tPREF\tf6=40000000\n"
        );
    }

    #[test]
    fn deletes_the_file_when_done() {
        let file = CookieFile::write(&[cookie(".youtube.com", "SID", "abc")]).unwrap();
        let path = file.path().to_path_buf();
        assert!(std::fs::read_to_string(&path).unwrap().contains("SID\tabc"));
        drop(file);
        assert!(!path.exists());
    }

    #[test]
    fn debug_hides_the_value() {
        assert!(!format!("{:?}", cookie(".x.com", "auth_token", "secret-value")).contains("secret-value"));
    }
}

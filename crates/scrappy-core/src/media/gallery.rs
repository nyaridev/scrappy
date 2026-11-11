//! Gallery: gallery-dl for posts, profiles and galleries. Listing reads what a link holds (`-J`, the first files
//! only, so a big profile answers quickly); saving puts the files in `Social/{site}/{user}/` as one job, skipping
//! files saved before (its archive in `.scrappy/`), with a temporary cookies file when there's a sign-in.

// ── Imports ───────────────────────────────
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::Value;

use super::download::{Found, Saved, record_file, relative};
use super::engine;
use super::probe::{PREVIEW_ITEMS, Probe};
use crate::jobs::Job;
use crate::tools::Toolbox;
use crate::vault::{Cookie, CookieFile};

// ── Constants ─────────────────────────────
const LIST_TIMEOUT: Duration = Duration::from_secs(120);
/// How many files a listing reads; a bigger profile shows "50+". Kept small: these sites limit request rates.
const LIST_LIMIT: u32 = 50;
const FOLDER: &str = "Social";
const ARCHIVE: &str = ".scrappy/gallery-dl.sqlite3";
const IMAGE_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "webp", "gif", "avif"];
/// gallery-dl's message types in `-J` output.
const MESSAGE_ERROR: i64 = -1;
const MESSAGE_URL: i64 = 3;
/// gallery-dl marks a file it skipped (already in the archive) with this.
const SKIPPED_MARK: &str = "# ";

// ── Types ─────────────────────────────────
/// What a link holds, from its first files.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Listing {
    /// gallery-dl's name for the site: `twitter`, `instagram`, `bluesky`.
    pub site: Option<String>,
    pub user: Option<String>,
    pub count: u32,
    /// There are more files than were listed.
    pub more: bool,
    /// The first files' names.
    pub items: Vec<String>,
    /// The first image, for a preview.
    pub preview: Option<String>,
}

/// Everything one gallery-dl run needs.
pub(super) struct GalleryOrder {
    pub root: PathBuf,
    pub url: String,
    /// Save only the first this many files.
    pub limit: Option<u32>,
    /// How many files there are, when the listing saw them all.
    pub total: Option<u32>,
    pub cookies: Option<Vec<Cookie>>,
}

// ── Private helpers ───────────────────────
/// The account a file came from, under the names gallery-dl's sites use.
fn user_of(fields: &Value) -> Option<String> {
    let paths: [&[&str]; 6] =
        [&["author", "handle"], &["author", "name"], &["user", "name"], &["username"], &["owner", "username"], &["uploader"]];
    paths.iter().find_map(|path| {
        let value = path.iter().try_fold(fields, |value, key| value.get(key))?;
        value.as_str().map(str::to_string).or_else(|| value.as_u64().map(|n| n.to_string()))
    })
}

/// Reads `-J` output: the files, or the error gallery-dl reported.
fn parse_listing(json: &str) -> Result<Listing, String> {
    let messages: Vec<Value> = serde_json::from_str(json).map_err(|_| "gallery-dl's answer couldn't be read.".to_string())?;
    let mut listing = Listing { site: None, user: None, count: 0, more: false, items: Vec::new(), preview: None };
    for message in &messages {
        let kind = message.get(0).and_then(Value::as_i64);
        let fields = message.as_array().and_then(|parts| parts.last()).unwrap_or(&Value::Null);
        if kind == Some(MESSAGE_ERROR) {
            return Err(fields.get("message").and_then(Value::as_str).unwrap_or("gallery-dl failed.").to_string());
        }
        if kind != Some(MESSAGE_URL) {
            continue;
        }
        listing.count += 1;
        listing.site = listing.site.or_else(|| fields.get("category").and_then(Value::as_str).map(str::to_string));
        listing.user = listing.user.or_else(|| user_of(fields));
        let name = fields.get("filename").and_then(Value::as_str).unwrap_or_default();
        let ext = fields.get("extension").and_then(Value::as_str).unwrap_or_default();
        if listing.items.len() < PREVIEW_ITEMS && !name.is_empty() {
            listing.items.push(if ext.is_empty() { name.to_string() } else { format!("{name}.{ext}") });
        }
        if listing.preview.is_none() && IMAGE_EXTENSIONS.contains(&ext.to_lowercase().as_str()) {
            listing.preview = message.get(1).and_then(Value::as_str).map(str::to_string);
        }
    }
    listing.more = listing.count >= LIST_LIMIT;
    Ok(listing)
}

/// The site and account folders of a saved file: `<root>/Social/<site>/<user>/file`.
fn site_and_user(root: &Path, file: &Path) -> (Option<String>, Option<String>) {
    let parts: Vec<String> = file
        .strip_prefix(root.join(FOLDER))
        .map(|rest| rest.iter().map(|part| part.to_string_lossy().into_owned()).collect())
        .unwrap_or_default();
    match parts.as_slice() {
        [site, user, _, ..] => (Some(site.clone()), Some(user.clone())),
        [site, _] => (Some(site.clone()), None),
        _ => (None, None),
    }
}

// ── Public API ────────────────────────────
impl Listing {
    /// What the window shows, with the preview already fetched.
    pub(super) fn probe(&self, url: &str, thumbnail: Option<String>) -> Probe {
        let title = match (&self.user, &self.site) {
            (Some(user), _) => user.clone(),
            (None, Some(site)) => site.clone(),
            (None, None) => url.to_string(),
        };
        Probe {
            url: url.to_string(),
            title,
            channel: None,
            duration: None,
            site: self.site.clone(),
            thumbnail,
            heights: Vec::new(),
            count: Some(self.count),
            more: self.more,
            files: true,
            items: self.items.clone(),
            sections: Vec::new(),
            entries: Vec::new(),
        }
    }
}

/// Lists the first files `url` holds.
pub(super) async fn list(toolbox: &Toolbox, url: &str, cookies: Option<&CookieFile>) -> Result<Listing, String> {
    let mut command = engine::gallery_dl(toolbox)?;
    engine::add_cookies(&mut command, cookies);
    command.args(["--range", &format!("1-{LIST_LIMIT}"), "-J", "--", url]);
    let output = tokio::time::timeout(LIST_TIMEOUT, command.output())
        .await
        .map_err(|_| "gallery-dl took too long to answer.".to_string())?
        .map_err(|e| format!("gallery-dl didn't start: {e}"))?;
    if !output.status.success() && output.stdout.is_empty() {
        let lines: Vec<String> = String::from_utf8_lossy(&output.stderr).lines().map(String::from).collect();
        return Err(engine::error_text(&lines));
    }
    parse_listing(&String::from_utf8_lossy(&output.stdout))
}

/// Saves the files `order` points at and records each one. Fails only when nothing was saved or skipped.
pub(super) async fn run(toolbox: &Toolbox, order: GalleryOrder, job: &Job) -> Result<Saved, String> {
    // Kept until this function returns, so the file is deleted when gallery-dl is done, stopped or failed.
    let cookie_file = order.cookies.as_deref().map(CookieFile::write).transpose()?;
    let mut command = engine::gallery_dl(toolbox)?;
    engine::add_cookies(&mut command, cookie_file.as_ref());
    command.arg("-d").arg(order.root.join(FOLDER)).arg("--download-archive").arg(order.root.join(ARCHIVE));
    if let Some(limit) = order.limit {
        command.args(["--range", &format!("1-{limit}")]);
    }
    command.args(["--", &order.url]);

    let total = order.total.or(order.limit).map(u64::from);
    let (mut files, mut skipped, mut folder, mut failed) = (0u32, 0u32, None, None);
    let (ok, errors) = engine::run_lines(command, "gallery-dl", job, |line| {
        let (path, saved) = match line.strip_prefix(SKIPPED_MARK) {
            Some(path) => (PathBuf::from(path.trim()), false),
            None => (PathBuf::from(line.trim()), true),
        };
        if !path.is_absolute() {
            return;
        }
        if saved {
            let (site, user) = site_and_user(&order.root, &path);
            let found = Found { kind: "social", url: &order.url, site, user, transferred: None };
            match record_file(&order.root, &path, found, None) {
                Ok(_) => files += 1,
                Err(error) => failed = Some(error),
            }
        } else {
            skipped += 1;
        }
        folder = folder.take().or_else(|| path.parent().map(|dir| relative(&order.root, dir)));
        let name = path.file_name().map(|name| name.to_string_lossy().into_owned());
        job.items(u64::from(files + skipped), total, name);
    })
    .await?;
    if let Some(error) = failed {
        return Err(error);
    }
    if files + skipped == 0 {
        return Err(if ok { "There was nothing to download.".into() } else { engine::error_text(&errors) });
    }
    tracing::info!("Saved {files} files with gallery-dl ({skipped} already saved)");
    Ok(Saved { path: folder, files: Some(files), skipped: Some(skipped) })
}

#[cfg(test)]
mod tests {
    use super::{parse_listing, site_and_user};
    use std::path::Path;

    #[test]
    fn reads_a_listing() {
        let json = r#"[
            [2, {"category": "bluesky", "author": {"handle": "bsky.app"}}],
            [3, "https://cdn.example/a", {"category": "bluesky", "author": {"handle": "bsky.app"},
                "filename": "a1", "extension": "jpeg"}],
            [3, "https://cdn.example/b", {"category": "bluesky", "filename": "b2", "extension": "mp4"}]
        ]"#;
        let listing = parse_listing(json).unwrap();
        assert_eq!((listing.count, listing.more), (2, false));
        assert_eq!(listing.site.as_deref(), Some("bluesky"));
        assert_eq!(listing.user.as_deref(), Some("bsky.app"));
        assert_eq!(listing.items, ["a1.jpeg", "b2.mp4"]);
        assert_eq!(listing.preview.as_deref(), Some("https://cdn.example/a"));
    }

    #[test]
    fn passes_on_gallery_dls_error() {
        let json = r#"[[-1, {"error": "AuthRequired", "message": "authenticated cookies needed to access this timeline"}]]"#;
        assert_eq!(parse_listing(json).unwrap_err(), "authenticated cookies needed to access this timeline");
    }

    #[test]
    fn finds_the_site_and_account_folders() {
        let root = Path::new("D:/Lib");
        let file = root.join("Social").join("bluesky").join("bsky.app").join("a.jpg");
        assert_eq!(site_and_user(root, &file), (Some("bluesky".into()), Some("bsky.app".into())));
        assert_eq!(site_and_user(root, &root.join("Videos").join("a.mp4")), (None, None));
    }
}

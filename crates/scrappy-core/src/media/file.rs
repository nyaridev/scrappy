//! File: one link fetched straight into a library folder, as a job with progress, then recorded in the catalog:
//! an image the extension sends (into `Images/{site}/`, images only) or a file Grab picked. The site comes from the
//! page it was found on.

// ── Imports ───────────────────────────────
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use reqwest::Url;
use tokio::io::AsyncWriteExt;

use super::download::{Taken, relative, reserve};
use crate::app::USER_AGENT;
use crate::jobs::{Job, JobPhase};
use crate::library::{self, template};
use crate::vault;

// ── Constants ─────────────────────────────
const IMAGE_FOLDER: &str = "Images/{site}";
const MAX_IMAGE_BYTES: u64 = 500_000_000;
/// Grab saves any file; this only stops a link that never ends.
const MAX_FILE_BYTES: u64 = 50_000_000_000;
const TIMEOUT: Duration = Duration::from_secs(30);
/// The error a file under its order's smallest size ends with.
pub(super) const TOO_SMALL: &str = "Smaller than the smallest size.";
/// Words at the end of an address that aren't the file's name: `…/Cat.png/revision/latest`.
const FILLER: &[&str] = &["latest", "revision", "original", "orig", "full", "raw", "download"];

// ── Types ─────────────────────────────────
/// Everything one file download needs.
pub(super) struct FileOrder {
    pub root: PathBuf,
    pub url: String,
    pub page_url: Option<String>,
    /// Allowed request headers only (`bridge` checked them).
    pub headers: Vec<(String, String)>,
    pub taken: Taken,
    /// What it is: `image` (only images are taken, as an extension send) or a Grab find's kind.
    pub kind: &'static str,
    /// Where it goes, relative to the library root; `None` for `Images/{site}`.
    pub folder: Option<PathBuf>,
    /// A smaller file isn't kept (`TOO_SMALL`).
    pub min_bytes: u64,
    /// Reports no progress: it runs inside a job that reports its own (a crawl).
    pub quiet: bool,
}

// ── Private helpers ───────────────────────
/// A site's short name for its folder: `www.instagram.com` → `instagram`, `bbc.co.uk` → `bbc`.
fn site_of(url: &str) -> Option<String> {
    let site = vault::site_of(url)?;
    Some(site.split('.').next().unwrap_or(&site).to_string())
}

/// The extension of one part of an address: `Clip.MP4` → `mp4`; `1.2.3` and `watch` have none.
fn segment_extension(segment: &str) -> Option<String> {
    let (_, ext) = segment.rsplit_once('.')?;
    let ok = (1..=5).contains(&ext.len())
        && ext.chars().all(|c| c.is_ascii_alphanumeric())
        && ext.chars().any(|c| c.is_ascii_alphabetic());
    ok.then(|| ext.to_ascii_lowercase())
}

/// The file name in the link without its extension, decoded: `.../Sunset%20sky.jpg?w=2` → `Sunset sky`. When the
/// last part has no extension, an earlier part with one wins (`…/Cat.png/revision/latest/50` → `Cat`), else the last
/// part that isn't filler or a number.
fn name_of(url: &str) -> Option<String> {
    let url = Url::parse(url).ok()?;
    let segments: Vec<&str> = url.path_segments()?.filter(|segment| !segment.is_empty()).collect();
    let named = segments.iter().rev().find(|segment| segment_extension(segment).is_some());
    let plain = || {
        let filler = |segment: &&&str| {
            FILLER.contains(&segment.to_ascii_lowercase().as_str()) || segment.chars().all(|c| c.is_ascii_digit())
        };
        segments.iter().rev().find(|segment| !filler(segment)).or(segments.last())
    };
    let last = named.or_else(plain)?;
    let decoded = String::from_utf8(percent_decode(last)).ok()?;
    let stem = Path::new(&decoded).file_stem()?.to_string_lossy().trim().to_string();
    (!stem.is_empty()).then_some(stem)
}

fn percent_decode(text: &str) -> Vec<u8> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        let hex = bytes.get(index + 1..index + 3).and_then(|h| std::str::from_utf8(h).ok());
        match (bytes[index], hex.and_then(|h| u8::from_str_radix(h, 16).ok())) {
            (b'%', Some(byte)) => {
                out.push(byte);
                index += 3;
            }
            (byte, _) => {
                out.push(byte);
                index += 1;
            }
        }
    }
    out
}

/// The extension for a response type, for a link whose name has none.
fn extension_for(mime: &str) -> Option<&'static str> {
    Some(match mime {
        "video/mp4" => "mp4",
        "video/webm" => "webm",
        "video/quicktime" => "mov",
        "audio/mpeg" => "mp3",
        "audio/mp4" => "m4a",
        "audio/ogg" => "ogg",
        "audio/wav" | "audio/x-wav" => "wav",
        "application/pdf" => "pdf",
        "application/zip" => "zip",
        "image/jpeg" | "image/jpg" | "image/pjpeg" => "jpg",
        "image/png" => "png",
        "image/webp" => "webp",
        "image/gif" => "gif",
        "image/avif" => "avif",
        "image/svg+xml" => "svg",
        "image/bmp" => "bmp",
        "image/tiff" => "tiff",
        "image/x-icon" | "image/vnd.microsoft.icon" => "ico",
        "image/heic" => "heic",
        _ => return None,
    })
}

/// The extension in the link's file name: `.../clip.MP4?x=1` → `mp4`.
fn extension_in(url: &str) -> Option<String> {
    let url = Url::parse(url).ok()?;
    let last = url.path_segments()?.rev().find(|segment| !segment.is_empty())?;
    segment_extension(last)
}

async fn fetch(order: &FileOrder, base: &Path, job: &Job) -> Result<(PathBuf, u64), String> {
    let client = reqwest::Client::builder()
        .connect_timeout(TIMEOUT)
        .read_timeout(TIMEOUT)
        .user_agent(USER_AGENT)
        .build()
        .map_err(|e| format!("Can't start the download: {e}"))?;
    let mut request = client.get(&order.url);
    for (name, value) in &order.headers {
        request = request.header(name.as_str(), value.as_str());
    }
    let mut response = request
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .map_err(|e| format!("The file didn't download: {}", e.without_url()))?;
    let mime = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.split(';').next().unwrap_or(value).trim().to_lowercase())
        .unwrap_or_default();
    let images_only = order.folder.is_none();
    let (ext, limit, too_big) = if images_only {
        let ext = extension_for(&mime).filter(|_| mime.starts_with("image/")).ok_or("This link isn't an image.")?;
        (ext.to_string(), MAX_IMAGE_BYTES, "The image is larger than 500 MB.")
    } else {
        // A link that looked like a picture or a clip but answers with a web page (a wiki's `File:Cat.png` page).
        if order.kind != "file" && matches!(mime.as_str(), "text/html" | "application/xhtml+xml") {
            return Err("The link gave a web page, not a file.".into());
        }
        // An image's type beats its link: sites send WebP for a `.png` address.
        let typed = extension_for(&mime).filter(|_| order.kind == "image" && mime.starts_with("image/"));
        let ext = typed
            .map(str::to_string)
            .or_else(|| extension_in(&order.url))
            .or_else(|| extension_for(&mime).map(str::to_string));
        (ext.unwrap_or_else(|| "bin".into()), MAX_FILE_BYTES, "The file is larger than 50 GB.")
    };
    let total = response.content_length();
    if total.is_some_and(|total| total > limit) {
        return Err(too_big.into());
    }
    if total.is_some_and(|total| total < order.min_bytes) {
        return Err(TOO_SMALL.into());
    }

    // Appended, not `with_extension`: a name like `v1.2` has no extension to replace.
    let target = PathBuf::from(format!("{}.{ext}", base.display()));
    let part = PathBuf::from(format!("{}.{ext}.part", base.display()));
    let mut file = tokio::fs::File::create(&part).await.map_err(|e| format!("Can't save the file: {e}"))?;
    let mut done = 0u64;
    let written: Result<(), String> = async {
        while let Some(chunk) = response.chunk().await.map_err(|e| format!("The file didn't download: {e}"))? {
            if job.cancelled() {
                return Err("Cancelled.".into());
            }
            done += chunk.len() as u64;
            if done > limit {
                return Err(too_big.into());
            }
            file.write_all(&chunk).await.map_err(|e| format!("Can't save the file: {e}"))?;
            if !order.quiet {
                job.progress(JobPhase::Download, done, total);
            }
        }
        file.flush().await.map_err(|e| format!("Can't save the file: {e}"))
    }
    .await;
    drop(file);
    let written = written.and_then(|()| if done < order.min_bytes { Err(TOO_SMALL.into()) } else { Ok(()) });
    if let Err(error) = written {
        let _ = fs::remove_file(&part);
        return Err(error);
    }
    fs::rename(&part, &target).map_err(|e| format!("Can't save the file: {e}"))?;
    Ok((target, done))
}

// ── Public API ────────────────────────────
/// Downloads the file into the library and records it. Returns its path relative to the library root.
pub(super) async fn run(order: FileOrder, job: &Job) -> Result<String, String> {
    let site = order.page_url.as_deref().and_then(site_of).or_else(|| site_of(&order.url));
    let title = name_of(&order.url).unwrap_or_else(|| order.kind.into());
    let fields = template::Fields { site: site.clone(), ..template::Fields::default() };
    let folder = order.folder.clone().unwrap_or_else(|| template::render(IMAGE_FOLDER, &fields));
    let name = template::render("{title}", &template::Fields { title: Some(title), ..template::Fields::default() });
    let reservation = reserve(&order.taken, &order.root.join(folder).join(name));
    if let Some(dir) = reservation.base.parent() {
        fs::create_dir_all(dir).map_err(|e| format!("Can't create the folder: {e}"))?;
    }
    let (file, bytes) = fetch(&order, &reservation.base, job).await?;
    let path = relative(&order.root, &file);
    library::record(
        &order.root,
        &library::Record {
            path: path.clone(),
            kind: order.kind,
            source_url: order.url,
            site,
            user: None,
            size_bytes: bytes,
            duration_secs: None,
            codec: None,
            transferred_bytes: bytes,
        },
    )?;
    tracing::info!("Downloaded a file ({})", order.kind);
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::{FileOrder, extension_for, extension_in, name_of, run, site_of};
    use crate::crawl::test_site::serve;
    use crate::jobs::Jobs;
    use std::sync::{Arc, Mutex};

    /// A wiki: a `.png` link that answers with its page, and one that answers with WebP.
    fn route(path: &str, _: &str) -> Option<(&'static str, Vec<u8>)> {
        Some(match path {
            "/wiki/File:Cat.png" => {
                ("text/html; charset=UTF-8", b"<!DOCTYPE html><title>File:Cat.png</title>".to_vec())
            }
            "/images/Cat.png/revision/latest" => ("image/webp", b"RIFF   WEBPVP8 ".to_vec()),
            _ => return None,
        })
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn keeps_only_real_media_and_names_it_by_its_type() {
        let address = serve(route).await;
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().to_path_buf();
        std::fs::create_dir_all(root.join(".scrappy")).unwrap();
        let order = |url: String| FileOrder {
            root: root.clone(),
            url,
            page_url: None,
            headers: Vec::new(),
            taken: Arc::new(Mutex::new(Default::default())),
            kind: "image",
            folder: Some("Images/wiki".into()),
            min_bytes: 0,
            quiet: true,
        };
        let results = Arc::new(Mutex::new(Vec::new()));
        let jobs = Jobs::new(Arc::new(|_| {}));
        let (page, picture) = (
            order(format!("{address}wiki/File:Cat.png")),
            order(format!("{address}images/Cat.png/revision/latest")),
        );
        let sink = results.clone();
        jobs.start(move |job| async move {
            let first = run(page, &job).await;
            let second = run(picture, &job).await;
            sink.lock().unwrap().extend([first, second]);
            Ok(())
        });
        while results.lock().unwrap().len() < 2 {
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
        let results = results.lock().unwrap();
        assert!(results[0].as_ref().unwrap_err().contains("web page"));
        assert_eq!(results[1].as_deref(), Ok("Images/wiki/Cat.webp"));
        assert!(!dir.path().join("Images/wiki/File_Cat.png").exists());
    }

    #[test]
    fn names_the_site() {
        assert_eq!(site_of("https://www.instagram.com/p/x").as_deref(), Some("instagram"));
        assert_eq!(site_of("https://pbs.twimg.com/media/a.jpg").as_deref(), Some("twimg"));
        assert_eq!(site_of("https://www.bbc.co.uk/news").as_deref(), Some("bbc"));
        assert_eq!(site_of("http://localhost:8080/a.png").as_deref(), Some("localhost"));
        assert_eq!(site_of("not a link"), None);
    }

    #[test]
    fn names_the_file_from_the_link() {
        assert_eq!(name_of("https://a.com/img/Sunset%20sky.jpg?w=2").as_deref(), Some("Sunset sky"));
        assert_eq!(name_of("https://a.com/media/abc123/").as_deref(), Some("abc123"));
        assert_eq!(name_of("https://a.com/"), None);
        assert_eq!(name_of("https://a.com/%FF%FE.png"), None);
        let wiki = "https://static.wikia.nocookie.net/w/images/a/ab/Sword.png/revision/latest/scale-to-width-down/50?cb=1";
        assert_eq!(name_of(wiki).as_deref(), Some("Sword"));
        let logo = "https://static.wikia.nocookie.net/w/images/3/39/Site-community-image/revision/latest?cb=1";
        assert_eq!(name_of(logo).as_deref(), Some("Site-community-image"));
    }

    #[test]
    fn names_the_extension() {
        assert_eq!(extension_for("image/jpeg"), Some("jpg"));
        assert_eq!(extension_for("video/mp4"), Some("mp4"));
        assert_eq!(extension_for("text/html"), None);
        assert_eq!(extension_in("https://a.com/v/Clip.MP4?x=1").as_deref(), Some("mp4"));
        assert_eq!(extension_in("https://a.com/v/1.2.3/"), None);
        assert_eq!(extension_in("https://a.com/watch"), None);
    }
}

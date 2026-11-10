//! Probe: a link's details from yt-dlp's `-J`: title, channel, length, thumbnail and the video heights on offer,
//! or for a playlist or channel, how many videos it holds and the first ones' titles.
//! The thumbnail comes through Rust as a data URL, so the window never loads a remote page (ADR-0005).

// ── Imports ───────────────────────────────
use std::time::Duration;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::engine;
use crate::app::USER_AGENT;
use crate::library::template::Fields;
use crate::tools::Toolbox;
use crate::vault::CookieFile;

// ── Constants ─────────────────────────────
const PROBE_TIMEOUT: Duration = Duration::from_secs(90);
const THUMBNAIL_TIMEOUT: Duration = Duration::from_secs(10);
const THUMBNAIL_MAX_BYTES: usize = 2_000_000;
/// The smallest thumbnail at least this wide is fetched: enough for the window's cards, and light to keep.
const THUMBNAIL_WIDTH: u32 = 320;
/// How many entries of a playlist or channel a probe lists; a bigger one shows "50+".
pub(super) const LIST_LIMIT: u32 = 50;
/// How many entries' titles the window shows.
pub(super) const PREVIEW_ITEMS: usize = 5;
/// A playlist's videos differ, so its quality choice offers the common heights.
const PLAYLIST_HEIGHTS: &[u32] = &[2160, 1440, 1080, 720, 480, 360];

// ── Types ─────────────────────────────────
/// What the window shows about a link before it's downloaded.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Probe {
    pub url: String,
    pub title: String,
    pub channel: Option<String>,
    /// Seconds.
    pub duration: Option<f64>,
    pub site: Option<String>,
    /// A `data:` URL.
    pub thumbnail: Option<String>,
    /// The video heights on offer, highest first; empty when the link has no video.
    pub heights: Vec<u32>,
    /// For a playlist, channel, post or profile: how many entries it holds. `None` for one video.
    pub count: Option<u32>,
    /// There are more entries than `count`: listing stopped there.
    pub more: bool,
    /// gallery-dl saves the files as the site has them: presets and quality don't apply.
    pub files: bool,
    /// The first entries' titles or file names.
    pub items: Vec<String>,
}

/// The parts of yt-dlp's JSON Scrappy reads.
#[derive(Debug, Deserialize)]
pub(super) struct Info {
    id: String,
    title: Option<String>,
    channel: Option<String>,
    uploader: Option<String>,
    duration: Option<f64>,
    upload_date: Option<String>,
    extractor_key: Option<String>,
    thumbnail: Option<String>,
    #[serde(default)]
    thumbnails: Vec<Thumbnail>,
    #[serde(rename = "_type")]
    kind: Option<String>,
    #[serde(default)]
    formats: Vec<Format>,
    playlist_count: Option<u32>,
    #[serde(default)]
    entries: Vec<Entry>,
}

/// One video of a playlist, as `--flat-playlist` lists it.
#[derive(Debug, Deserialize)]
struct Entry {
    id: Option<String>,
    title: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Thumbnail {
    url: String,
    width: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct Format {
    vcodec: Option<String>,
    height: Option<u32>,
}

// ── Public API ────────────────────────────
impl Info {
    /// Reads yt-dlp's JSON for one video or a playlist.
    pub(super) fn parse(json: &str) -> Result<Self, String> {
        serde_json::from_str(json).map_err(|e| format!("yt-dlp's answer couldn't be read: {e}"))
    }

    /// Whether this is a playlist or channel rather than one video.
    pub(super) fn is_playlist(&self) -> bool {
        self.kind.as_deref() == Some("playlist")
    }

    /// The title, or the id when there's none.
    pub(super) fn title(&self) -> String {
        self.title.clone().unwrap_or_else(|| self.id.clone())
    }

    /// The site's name in lowercase: `youtube`, `twitter`.
    pub(super) fn site(&self) -> Option<String> {
        self.extractor_key.as_ref().map(|key| key.to_lowercase())
    }

    /// The facts a path template uses. The channel falls back to the uploader, as many sites only have one.
    pub(super) fn fields(&self) -> Fields {
        Fields {
            title: Some(self.title()),
            id: Some(self.id.clone()),
            channel: self.channel.clone().or_else(|| self.uploader.clone()),
            playlist: None,
            upload_date: self.upload_date.clone(),
            site: self.site(),
            user: self.uploader.clone(),
        }
    }

    /// What the window shows, with the thumbnail already fetched.
    pub(super) fn probe(&self, url: &str, thumbnail: Option<String>) -> Probe {
        let mut heights: Vec<u32> = self
            .formats
            .iter()
            .filter(|f| f.vcodec.as_deref().is_some_and(|codec| codec != "none"))
            .filter_map(|f| f.height)
            .collect();
        heights.sort_unstable_by(|a, b| b.cmp(a));
        heights.dedup();
        let listed = u32::try_from(self.entries.len()).unwrap_or(u32::MAX);
        let (count, more) = match self.is_playlist() {
            true => (Some(self.playlist_count.unwrap_or(listed)), self.playlist_count.is_none() && listed >= LIST_LIMIT),
            false => (None, false),
        };
        if self.is_playlist() {
            heights = PLAYLIST_HEIGHTS.to_vec();
        }
        let items = self.entries.iter().take(PREVIEW_ITEMS).filter_map(|e| e.title.clone().or_else(|| e.id.clone()));
        Probe {
            url: url.to_string(),
            title: self.title(),
            channel: self.fields().channel,
            duration: self.duration,
            site: self.site(),
            thumbnail,
            heights,
            count,
            more,
            files: false,
            items: items.collect(),
        }
    }

    /// The smallest thumbnail at least 320 pixels wide, else yt-dlp's pick.
    pub(super) fn thumbnail_url(&self) -> Option<&str> {
        self.thumbnails
            .iter()
            .filter(|t| t.width.is_some_and(|width| width >= THUMBNAIL_WIDTH))
            .min_by_key(|t| t.width)
            .map(|t| t.url.as_str())
            .or(self.thumbnail.as_deref())
    }
}

/// Runs `yt-dlp -J` on `url` (already checked), with the page's allowed headers and a sign-in when there is one,
/// and returns its JSON.
pub(super) async fn fetch(
    toolbox: &Toolbox,
    url: &str,
    headers: &[(String, String)],
    cookies: Option<&CookieFile>,
) -> Result<String, String> {
    let mut command = engine::yt_dlp(toolbox)?;
    engine::add_headers(&mut command, headers);
    engine::add_cookies(&mut command, cookies);
    // A playlist or channel is listed, not resolved video by video; one video still comes with its formats.
    command.args(["--flat-playlist", "--playlist-items", &format!("1-{LIST_LIMIT}"), "-J", "--", url]);
    let output = tokio::time::timeout(PROBE_TIMEOUT, command.output())
        .await
        .map_err(|_| "yt-dlp took too long to answer.".to_string())?
        .map_err(|e| format!("yt-dlp didn't start: {e}"))?;
    if !output.status.success() {
        let lines: Vec<String> = String::from_utf8_lossy(&output.stderr).lines().map(String::from).collect();
        return Err(engine::error_text(&lines));
    }
    String::from_utf8(output.stdout).map_err(|_| "yt-dlp's answer couldn't be read.".into())
}

/// Downloads a thumbnail and returns it as a `data:` URL; `None` when there's none or it fails.
pub(super) async fn thumbnail(url: Option<&str>) -> Option<String> {
    let url = engine::check_url(url?).ok()?;
    let client = reqwest::Client::builder().timeout(THUMBNAIL_TIMEOUT).user_agent(USER_AGENT).build().ok()?;
    let response = client.get(url).send().await.ok()?.error_for_status().ok()?;
    let mime = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.split(';').next().unwrap_or(value).trim().to_string())
        .filter(|mime| mime.starts_with("image/"))
        .unwrap_or_else(|| "image/jpeg".into());
    let bytes = response.bytes().await.ok()?;
    (bytes.len() <= THUMBNAIL_MAX_BYTES).then(|| format!("data:{mime};base64,{}", STANDARD.encode(&bytes)))
}

#[cfg(test)]
mod tests {
    use super::Info;

    const SAMPLE: &str = r#"{"id":"jNQXAC9IVRw","title":"Me at the zoo","channel":null,"uploader":"jawed",
        "duration":19,"upload_date":"20050424","extractor_key":"Youtube","_type":"video","formats":[
        {"vcodec":"avc1.4D400B","acodec":"none","height":144},{"vcodec":"none","acodec":"mp4a.40.2","height":null},
        {"vcodec":"vp9","height":240},{"vcodec":"avc1.42001E","height":240}]}"#;

    #[test]
    fn reads_a_video() {
        let info = Info::parse(SAMPLE).unwrap();
        let probe = info.probe("https://youtu.be/jNQXAC9IVRw", None);
        assert_eq!(probe.title, "Me at the zoo");
        assert_eq!(probe.channel.as_deref(), Some("jawed"));
        assert_eq!(probe.site.as_deref(), Some("youtube"));
        assert_eq!(probe.heights, vec![240, 144]);
        assert_eq!(info.fields().upload_date.as_deref(), Some("20050424"));
    }

    #[test]
    fn picks_a_small_thumbnail() {
        let json = r#"{"id":"a","thumbnail":"https://i.ytimg.com/maxres.jpg","thumbnails":[
            {"url":"https://i.ytimg.com/default.jpg","width":120},{"url":"https://i.ytimg.com/maxres.jpg","width":1280},
            {"url":"https://i.ytimg.com/mq.jpg","width":320},{"url":"https://i.ytimg.com/hq.jpg","width":480},
            {"url":"https://i.ytimg.com/unknown.jpg"}]}"#;
        assert_eq!(Info::parse(json).unwrap().thumbnail_url(), Some("https://i.ytimg.com/mq.jpg"));
        let only_pick = r#"{"id":"a","thumbnail":"https://example.com/t.jpg"}"#;
        assert_eq!(Info::parse(only_pick).unwrap().thumbnail_url(), Some("https://example.com/t.jpg"));
    }

    #[test]
    fn reads_a_playlist() {
        let json = r#"{"id":"PL1","_type":"playlist","title":"Tutorials","channel":"Tim","playlist_count":10,
            "entries":[{"id":"a","title":"One"},{"id":"b","title":null}]}"#;
        let probe = Info::parse(json).unwrap().probe("https://youtube.com/playlist?list=PL1", None);
        assert_eq!((probe.count, probe.more, probe.files), (Some(10), false, false));
        assert_eq!(probe.items, ["One", "b"]);
        assert_eq!(probe.heights[0], 2160);

        // A channel too big to list whole has no count: the listing's size, and more.
        let entries: Vec<String> = (0..50).map(|n| format!(r#"{{"id":"v{n}"}}"#)).collect();
        let json = format!(r#"{{"id":"UC1","_type":"playlist","entries":[{}]}}"#, entries.join(","));
        let probe = Info::parse(&json).unwrap().probe("https://youtube.com/@a/videos", None);
        assert_eq!((probe.count, probe.more), (Some(50), true));
        assert_eq!(Info::parse(SAMPLE).unwrap().probe("u", None).count, None);
    }
}

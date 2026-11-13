//! Probe: a link's details from yt-dlp's `-J`: title, channel, length, thumbnail and the video heights on offer,
//! or for a playlist or channel, how many videos it holds and the first ones' titles. A channel whose listing holds
//! lists (its Videos, Shorts and Live tabs) gets them as sections, each counted, and every list names its videos so
//! the window can leave some out (ADR-0016).
//! The thumbnail comes through Rust as a data URL, so the window never loads a remote page (ADR-0005).

// ── Imports ───────────────────────────────
use std::time::Duration;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use reqwest::Url;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::engine;
use crate::net;
use crate::library::template::Fields;
use crate::tools::Toolbox;
use crate::vault::{self, CookieFile};

// ── Constants ─────────────────────────────
const PROBE_TIMEOUT: Duration = Duration::from_secs(90);
const THUMBNAIL_TIMEOUT: Duration = Duration::from_secs(10);
const THUMBNAIL_MAX_BYTES: usize = 2_000_000;
/// yt-dlp's answer, with its generic reader off, for a page no site reader knows.
pub(super) const NO_READER: &str = "No reader knows this page.";
/// The smallest thumbnail at least this wide is fetched: enough for the window's cards, and light to keep.
const THUMBNAIL_WIDTH: u32 = 320;
/// How many entries of a playlist or channel (and of each section) a probe lists; a bigger one shows "1000+".
pub(super) const LIST_LIMIT: u32 = 1000;
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
    /// The lists a channel holds (Videos, Shorts…), each downloadable on its own; empty for anything else.
    pub sections: Vec<Section>,
    /// A playlist's videos as listed (up to 1000); empty for a channel (its sections have them) and for one video.
    pub entries: Vec<ListEntry>,
    /// For a list read from one of its videos (`watch?v=…&list=…`, a Mix too): that video's own link.
    pub video: Option<String>,
}

/// One video of a list, as the probe listed it.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ListEntry {
    /// Its link; also how a download leaves it out.
    pub url: String,
    pub title: String,
    /// Seconds.
    pub duration: Option<f64>,
    #[ts(type = "number | null")]
    pub views: Option<u64>,
    /// Its thumbnail is taller than wide (a short); the picture comes from `Downloads::preview`.
    pub tall: bool,
}

/// One list inside a channel's listing.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Section {
    pub url: String,
    /// `Videos`, `Shorts`: the list's title without the channel's name.
    pub title: String,
    pub count: u32,
    /// There are more entries than `count`: listing stopped there.
    pub more: bool,
    pub entries: Vec<ListEntry>,
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
    webpage_url: Option<String>,
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

/// One video of a playlist as `--flat-playlist` lists it, or a list inside a channel with its own entries.
#[derive(Debug, Deserialize)]
struct Entry {
    id: Option<String>,
    title: Option<String>,
    url: Option<String>,
    duration: Option<f64>,
    view_count: Option<u64>,
    thumbnail: Option<String>,
    #[serde(default)]
    thumbnails: Vec<Thumbnail>,
    #[serde(rename = "_type")]
    kind: Option<String>,
    webpage_url: Option<String>,
    playlist_count: Option<u32>,
    #[serde(default)]
    entries: Vec<Entry>,
}

#[derive(Debug, Deserialize)]
struct Thumbnail {
    url: String,
    width: Option<u32>,
    height: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct Format {
    vcodec: Option<String>,
    height: Option<u32>,
}

// ── Private helpers ───────────────────────
/// A video link that names a list (`watch?v=…&list=PL…`, or a Mix's `list=RD…`) is read as the list.
pub(super) fn names_playlist(url: &str) -> bool {
    let Ok(url) = Url::parse(url) else { return false };
    url.query_pairs().any(|(key, value)| key == "list" && !value.is_empty())
}

/// The one video of a link that names a list: the link without its list.
fn video_link(url: &str) -> Option<String> {
    let mut link = Url::parse(url).ok()?;
    if !names_playlist(url) || !link.query_pairs().any(|(key, _)| key == "v") {
        return None;
    }
    let kept: Vec<(String, String)> = link
        .query_pairs()
        .filter(|(key, _)| !matches!(key.as_ref(), "list" | "index" | "start_radio" | "pp"))
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect();
    link.query_pairs_mut().clear().extend_pairs(kept);
    Some(link.into())
}

/// A list's size: yt-dlp's count when it has one, else the entries listed, with "more" when listing stopped at the
/// limit.
fn size(playlist_count: Option<u32>, listed: usize) -> (u32, bool) {
    let listed = u32::try_from(listed).unwrap_or(u32::MAX);
    (playlist_count.unwrap_or(listed), playlist_count.is_none() && listed >= LIST_LIMIT)
}

/// The smallest thumbnail at least 320 pixels wide.
fn small_thumbnail(thumbnails: &[Thumbnail]) -> Option<&Thumbnail> {
    thumbnails.iter().filter(|t| t.width.is_some_and(|width| width >= THUMBNAIL_WIDTH)).min_by_key(|t| t.width)
}

/// A list's videos that have a link.
fn list_entries(entries: &[Entry]) -> Vec<ListEntry> {
    entries
        .iter()
        .filter_map(|entry| {
            let url = entry.url.clone()?;
            let title = entry.title.clone().or_else(|| entry.id.clone()).unwrap_or_else(|| url.clone());
            let tall = small_thumbnail(&entry.thumbnails).is_some_and(|t| t.height > t.width);
            Some(ListEntry { url, title, duration: entry.duration, views: entry.view_count, tall })
        })
        .collect()
}

impl Entry {
    /// A list with its own link, as a channel's tabs are.
    fn is_section(&self) -> bool {
        self.kind.as_deref() == Some("playlist") && self.webpage_url.is_some()
    }
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

    /// yt-dlp's generic reader read it: a plain file or stream, named after its link (`master`), not its page.
    pub(super) fn generic(&self) -> bool {
        self.extractor_key.as_deref() == Some("Generic")
    }

    /// The facts a path template uses. The channel falls back to the uploader, as many sites only have one. The site
    /// is the page's domain (`youtube.com`); none for a plain file or stream, whose link names only its server.
    pub(super) fn fields(&self) -> Fields {
        let site = (!self.generic()).then(|| self.webpage_url.as_deref().and_then(vault::site_of)).flatten();
        Fields {
            title: Some(self.title()),
            id: Some(self.id.clone()),
            channel: self.channel.clone().or_else(|| self.uploader.clone()),
            playlist: None,
            upload_date: self.upload_date.clone(),
            site: site.or_else(|| self.site().filter(|_| !self.generic())),
            user: self.uploader.clone(),
        }
    }

    /// Every format it offers is audio alone (SoundCloud, a podcast); `false` when a format has video or doesn't say.
    pub(super) fn audio_only(&self) -> bool {
        !self.formats.is_empty() && self.formats.iter().all(|f| f.vcodec.as_deref() == Some("none"))
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
        let sections = self.sections();
        let (count, more) = match (self.is_playlist(), sections.is_empty()) {
            (false, _) => (None, false),
            (true, true) => {
                let (count, more) = size(self.playlist_count, self.entries.len());
                (Some(count), more)
            }
            (true, false) => (Some(sections.iter().map(|s| s.count).sum()), sections.iter().any(|s| s.more)),
        };
        if self.is_playlist() {
            heights = PLAYLIST_HEIGHTS.to_vec();
        }
        // A channel's first entries are its sections' first videos.
        let videos = self.entries.iter().flat_map(|e| if e.is_section() { e.entries.iter().collect() } else { vec![e] });
        let items = videos.take(PREVIEW_ITEMS).filter_map(|e| e.title.clone().or_else(|| e.id.clone()));
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
            entries: if self.is_playlist() && sections.is_empty() { list_entries(&self.entries) } else { Vec::new() },
            sections,
            video: if self.is_playlist() { video_link(url) } else { None },
        }
    }

    /// The lists inside this listing, titled without the channel's name.
    fn sections(&self) -> Vec<Section> {
        let prefix = format!("{} - ", self.title());
        let sections = self.entries.iter().filter(|entry| entry.is_section());
        sections
            .map(|entry| {
                let (count, more) = size(entry.playlist_count, entry.entries.len());
                let title = entry.title.clone().unwrap_or_default();
                let title = title.strip_prefix(&prefix).map_or(title.clone(), str::to_string);
                let (url, entries) = (entry.webpage_url.clone().unwrap_or_default(), list_entries(&entry.entries));
                Section { url, title, count, more, entries }
            })
            .collect()
    }

    /// The smallest thumbnail at least 320 pixels wide, else yt-dlp's pick; a list without one (a Mix) takes its
    /// first video's.
    pub(super) fn thumbnail_url(&self) -> Option<&str> {
        let first = self.entries.iter().find(|entry| !entry.is_section()).and_then(|entry| {
            small_thumbnail(&entry.thumbnails).map(|t| t.url.as_str()).or(entry.thumbnail.as_deref())
        });
        small_thumbnail(&self.thumbnails).map(|t| t.url.as_str()).or(self.thumbnail.as_deref()).or(first)
    }

    /// Each listed video's link with its thumbnail's, sections' videos included.
    pub(super) fn previews(&self) -> Vec<(String, String)> {
        let videos = self.entries.iter().flat_map(|e| if e.is_section() { e.entries.iter().collect() } else { vec![e] });
        videos
            .filter_map(|entry| {
                let picture = small_thumbnail(&entry.thumbnails).map(|t| t.url.clone()).or(entry.thumbnail.clone());
                Some((entry.url.clone()?, picture?))
            })
            .collect()
    }
}

/// Runs `yt-dlp -J` on `url` (already checked), with the page's allowed headers and a sign-in when there is one,
/// and returns its JSON. `known_only` turns the generic reader off: a page no site reader knows fails at once with
/// `NO_READER` (ADR-0042).
pub(super) async fn fetch(
    toolbox: &Toolbox,
    url: &str,
    headers: &[(String, String)],
    cookies: Option<&CookieFile>,
    known_only: bool,
) -> Result<String, String> {
    let mut command = engine::yt_dlp(toolbox)?;
    engine::add_headers(&mut command, headers);
    engine::add_cookies(&mut command, cookies);
    if known_only {
        command.args(["--ies", "default,-generic"]);
    }
    // A playlist or channel is listed, not resolved video by video; one video still comes with its formats.
    if names_playlist(url) {
        // After the `--no-playlist` every yt-dlp run starts with; the later one wins.
        command.arg("--yes-playlist");
    }
    command.args(["--flat-playlist", "--playlist-items", &format!("1-{LIST_LIMIT}"), "-J", "--", url]);
    let output = tokio::time::timeout(PROBE_TIMEOUT, command.output())
        .await
        .map_err(|_| "yt-dlp took too long to answer.".to_string())?
        .map_err(|e| format!("yt-dlp didn't start: {e}"))?;
    if !output.status.success() {
        let lines: Vec<String> = String::from_utf8_lossy(&output.stderr).lines().map(String::from).collect();
        let error = engine::error_text(&lines);
        return Err(if error.starts_with("No suitable extractor") { NO_READER.into() } else { error });
    }
    String::from_utf8(output.stdout).map_err(|_| "yt-dlp's answer couldn't be read.".into())
}

/// The JSON of each section inside a channel's listing, with its link, so a section downloads from it.
pub(super) fn section_listings(json: &str) -> Vec<(String, String)> {
    let Ok(listing) = serde_json::from_str::<serde_json::Value>(json) else { return Vec::new() };
    let entries = listing.get("entries").and_then(serde_json::Value::as_array).map_or(&[][..], Vec::as_slice);
    entries
        .iter()
        .filter(|entry| entry.get("_type").and_then(serde_json::Value::as_str) == Some("playlist"))
        .filter_map(|entry| Some((entry.get("webpage_url")?.as_str()?.to_string(), entry.to_string())))
        .collect()
}

/// Downloads a thumbnail and returns it as a `data:` URL; `None` when there's none or it fails.
pub(super) async fn thumbnail(url: Option<&str>) -> Option<String> {
    let url = engine::check_url(url?).ok()?;
    let client = net::client().timeout(THUMBNAIL_TIMEOUT).build().ok()?;
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
    use super::{Info, names_playlist, video_link};

    #[test]
    fn is_audio_only_when_every_format_says_so() {
        let audio_only = |json: &str| Info::parse(json).unwrap().audio_only();
        assert!(audio_only(r#"{"id":"a","formats":[{"vcodec":"none"},{"vcodec":"none"}]}"#));
        // A format that doesn't name its codec may hold video (X, plain file links).
        assert!(!audio_only(r#"{"id":"b","formats":[{"vcodec":"none"},{"height":null}]}"#));
        assert!(!audio_only(r#"{"id":"c","formats":[{"vcodec":"avc1","height":720}]}"#));
        assert!(!audio_only(r#"{"id":"d"}"#));
    }

    #[test]
    fn reads_a_video_in_a_playlist_or_mix_as_the_list() {
        assert!(names_playlist("https://www.youtube.com/watch?v=cvaIgq5j2Q8&list=PL0wqt_um4x0bsdViTJBmnl6KGMoSqxfZy"));
        assert!(names_playlist("https://www.youtube.com/watch?v=cvaIgq5j2Q8&list=RDcvaIgq5j2Q8&start_radio=1"));
        assert!(!names_playlist("https://www.youtube.com/watch?v=cvaIgq5j2Q8"));
        assert!(!names_playlist("https://www.youtube.com/watch?v=cvaIgq5j2Q8&list="));
    }

    #[test]
    fn keeps_the_one_video_of_a_list_link() {
        let mix = "https://www.youtube.com/watch?v=Tx8Ml-k4Z-8&list=RDdgKjUrX5YKs&index=3&t=42";
        assert_eq!(video_link(mix).as_deref(), Some("https://www.youtube.com/watch?v=Tx8Ml-k4Z-8&t=42"));
        assert_eq!(video_link("https://www.youtube.com/playlist?list=PL1"), None);
        assert_eq!(video_link("https://www.youtube.com/watch?v=a"), None);
    }

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
        let mix = r#"{"id":"RD1","_type":"playlist","entries":[{"id":"a","url":"https://www.youtube.com/watch?v=a",
            "thumbnails":[{"url":"https://i.ytimg.com/a.jpg","width":336,"height":188}]}]}"#;
        assert_eq!(Info::parse(mix).unwrap().thumbnail_url(), Some("https://i.ytimg.com/a.jpg"));
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
        let entries: Vec<String> = (0..1000).map(|n| format!(r#"{{"id":"v{n}"}}"#)).collect();
        let json = format!(r#"{{"id":"UC1","_type":"playlist","entries":[{}]}}"#, entries.join(","));
        let probe = Info::parse(&json).unwrap().probe("https://youtube.com/@a/videos", None);
        assert_eq!((probe.count, probe.more), (Some(1000), true));
        assert_eq!(Info::parse(SAMPLE).unwrap().probe("u", None).count, None);
    }

    /// yt-dlp's listing of `youtube.com/@name`: its tabs, each a list with its own entries.
    const CHANNEL: &str = r#"{"id":"UC1","_type":"playlist","title":"GDULA","channel":"GDULA","playlist_count":2,
        "extractor_key":"YoutubeTab","entries":[
        {"id":"UC1","_type":"playlist","title":"GDULA - Videos","playlist_count":2,
         "webpage_url":"https://www.youtube.com/@GDULA7/videos","entries":[
            {"id":"a","title":"One","url":"https://www.youtube.com/watch?v=a","duration":61,"view_count":18000,
             "thumbnails":[{"url":"https://i.ytimg.com/a.jpg","width":360,"height":202}]},
            {"id":"b","title":"Two","url":"https://www.youtube.com/watch?v=b"}]},
        {"id":"UC1","_type":"playlist","title":"GDULA - Shorts","webpage_url":"https://www.youtube.com/@GDULA7/shorts",
         "entries":[{"id":"c","title":"Short","url":"https://www.youtube.com/shorts/c",
            "thumbnails":[{"url":"https://i.ytimg.com/c.jpg","width":405,"height":720}]}]}]}"#;

    #[test]
    fn counts_a_channels_sections() {
        let probe = Info::parse(CHANNEL).unwrap().probe("https://www.youtube.com/@GDULA7", None);
        assert_eq!((probe.count, probe.more), (Some(3), false));
        assert_eq!(probe.items, ["One", "Two", "Short"]);
        let sections: Vec<_> = probe.sections.iter().map(|s| (s.title.as_str(), s.count, s.url.as_str())).collect();
        assert_eq!(
            sections,
            [("Videos", 2, "https://www.youtube.com/@GDULA7/videos"), ("Shorts", 1, "https://www.youtube.com/@GDULA7/shorts")]
        );

        let videos = &probe.sections[0].entries;
        assert_eq!((videos[0].url.as_str(), videos[0].duration), ("https://www.youtube.com/watch?v=a", Some(61.0)));
        assert_eq!((videos[0].views, videos[1].views), (Some(18000), None));
        assert!(probe.entries.is_empty());
        assert_eq!((videos[0].tall, probe.sections[1].entries[0].tall), (false, true));
        let previews = Info::parse(CHANNEL).unwrap().previews();
        assert_eq!(previews[0], ("https://www.youtube.com/watch?v=a".into(), "https://i.ytimg.com/a.jpg".into()));
        assert_eq!(previews.len(), 2);

        let listings = super::section_listings(CHANNEL);
        assert_eq!(listings.len(), 2);
        let shorts = Info::parse(&listings[1].1).unwrap().probe(&listings[1].0, None);
        assert_eq!((shorts.title.as_str(), shorts.count, shorts.sections.len()), ("GDULA - Shorts", Some(1), 0));
        assert_eq!(shorts.entries[0].url, "https://www.youtube.com/shorts/c");
    }
}

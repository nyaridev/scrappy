//! Downloads: the download list (kept on disk when the app asks, ADR-0016), the queue downloads wait in (ADR-0010),
//! and the sign-ins they use: one given for this download, else the site's remembered one (ADR-0005). Each link's
//! probe comes from `probes`, so a download reuses it instead of asking the site again. A link becomes one video, a playlist
//! or a gallery-dl collection, as its route and probe say; a channel's picked sections become one download each.
//! The queue's size and the speed limit come from the settings (ADR-0017). Scrapes and hunts get rows too, through
//! `track`, and every row says where it came from (ADR-0018).

// ── Imports ───────────────────────────────
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::download::{self, Order, Saved, Taken, size_of, types_of};
use super::download_list::DownloadList;
use super::engine;
use super::gallery::{self, GalleryOrder};
use super::file::{self, FileOrder, TOO_SMALL};
use super::playlist::{self, PlaylistOrder};
use super::presets::{self, Preset};
use super::probe;
use super::probes::{Probed, Probes, classify};
use crate::jobs::{Job, JobId, Jobs, Queue};
use crate::library::template::{self, Fields};
use crate::settings::DownloadLimits;
use crate::tools::Toolbox;
use crate::vault::{Cookie, Vault};

// ── Constants ─────────────────────────────
/// The preset column for an image sent from the extension.
const IMAGE_PRESET: &str = "Image";
/// The preset column for a post or profile: gallery-dl saves the files as they are.
const FILES_PRESET: &str = "Original files";
/// The preset column for a file Grab picked, saved as it is.
const FILE_PRESET: &str = "Original file";
/// How many files of a playlist or profile a send from the extension saves.
const EXTENSION_LIMIT: u32 = 50;

// ── Types ─────────────────────────────────
/// What the window asks to download.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DownloadRequest {
    pub url: String,
    /// A preset's id.
    pub preset: String,
    /// The highest video height to take; `None` for the best.
    pub max_height: Option<u32>,
    /// For a playlist or profile: save only its first this many files (of each section); `None` for all.
    pub limit: Option<u32>,
    /// The probe's sections to download, one download each; empty for the whole link.
    #[serde(default)]
    pub sections: Vec<String>,
    /// Links of a list's videos (from the probe's entries) to leave out.
    #[serde(default)]
    pub skip: Vec<String>,
}

/// Where a download is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum DownloadState {
    Queued,
    Running,
    Done,
    Failed,
    Cancelled,
}

/// Where a download came from: the view or the extension that started it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum DownloadSource {
    /// The Download view; also rows kept from before sources were.
    #[default]
    Download,
    Grab,
    Scrape,
    Hunt,
    Extension,
}

/// One download in the list. Progress arrives as job events with the same id.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DownloadEntry {
    pub id: JobId,
    #[serde(default)]
    pub source: DownloadSource,
    /// The link; empty for a hunt.
    pub url: String,
    pub title: String,
    /// The preset's name.
    pub preset: String,
    pub state: DownloadState,
    /// A small `data:` URL from the probe.
    pub thumbnail: Option<String>,
    /// The saved file, or the folder a playlist or profile went to, relative to the library root. Until a single
    /// video is saved: the folder it goes to.
    pub file: Option<String>,
    /// For a playlist or profile: the files saved.
    pub files: Option<u32>,
    /// For a playlist or profile: the files skipped because they were saved before.
    pub skipped: Option<u32>,
    /// Bytes on disk of what it saved; `None` until it's done, and for rows kept from before sizes were.
    #[ts(type = "number | null")]
    pub size: Option<u64>,
    /// Why it failed; on a done row, a note on how it was saved (kept without audio).
    pub error: Option<String>,
    /// For files kept as they are (no preset): their types, most common first (`MP4`, `JPG`); shown in the preset's
    /// place.
    #[serde(default)]
    pub types: Vec<String>,
}

/// What a link comes with: the page's headers, a sign-in given for it, the row's source, and the page it was sent
/// from.
type Given<'a> = (Vec<(String, String)>, Option<Vec<Cookie>>, DownloadSource, Option<&'a Sent>);

/// What the extension sent with a link: the page and its title, which a plain stream is named after (ADR-0042),
/// and the preset a button picked (ADR-0043).
#[derive(Default)]
pub struct Sent {
    pub page_url: Option<String>,
    pub title: Option<String>,
    /// A preset's id; `None` for the start preset from Settings.
    pub preset: Option<String>,
}

/// A new row: where it came from, its link and title, and what the list's preset column shows.
pub struct Row {
    pub source: DownloadSource,
    pub url: String,
    pub title: String,
    /// The preset's name, or the kind of job ("Mirror", "Images").
    pub kind: String,
    pub thumbnail: Option<String>,
}

/// A link Grab picked, saved into `folder` as one job.
#[derive(Debug, Clone)]
pub struct SaveRequest {
    pub url: String,
    /// The page it was found on: sent as the Referer.
    pub page_url: String,
    /// `video`, `audio`, `image` or `file`, for the catalog.
    pub kind: &'static str,
    /// An HLS or DASH playlist: yt-dlp fetches its pieces and joins them, with the default preset.
    pub stream: bool,
    /// Relative to the library root.
    pub folder: PathBuf,
    /// The catalog's site.
    pub site: Option<String>,
    /// The list's title; a stream's file name too.
    pub title: String,
}

/// Why a link couldn't be queued.
#[derive(Debug, Clone, PartialEq)]
pub enum SendError {
    /// The site wants a signed-in visitor; `tried` when a sign-in was used and still wasn't enough.
    NeedsLogin { site: String, tried: bool },
    Failed(String),
}

/// The downloads and what they share.
#[derive(Clone)]
pub struct Downloads {
    toolbox: Toolbox,
    probes: Probes,
    queue: Queue,
    entries: Arc<Mutex<Vec<DownloadEntry>>>,
    list: DownloadList,
    taken: Taken,
}

// ── Private helpers ───────────────────────
/// A row for the list.
fn row(source: DownloadSource, url: String, title: String, kind: &str, thumbnail: Option<String>) -> Row {
    Row { source, url, title, kind: kind.to_string(), thumbnail }
}

/// Done, failed or stopped: it can leave the list.
fn finished(entry: &DownloadEntry) -> bool {
    !matches!(entry.state, DownloadState::Queued | DownloadState::Running)
}

impl From<String> for SendError {
    fn from(message: String) -> Self {
        Self::Failed(message)
    }
}


/// For a plain stream yt-dlp named after its link (`master`): its title, place and site from the page it was sent
/// from. `None` when yt-dlp knew the site, or the page doesn't say.
fn named(from: &Sent, info_json: &str, audio: bool) -> Option<(String, PathBuf, Option<String>)> {
    let page_url = from.page_url.as_deref()?;
    let info = probe::Info::parse(info_json).ok().filter(probe::Info::generic)?;
    let site = crate::vault::site_of(page_url);
    let title = from.title.clone().filter(|title| !title.trim().is_empty()).or_else(|| site.clone())?;
    let fields = Fields { title: Some(title.clone()), id: None, site: site.clone(), ..info.fields() };
    Some((title, template::render(&engine::place_template(audio), &fields), site))
}

// ── Public API ────────────────────────────
impl SendError {
    /// What the window shows.
    pub fn message(&self) -> String {
        match self {
            Self::NeedsLogin { site, tried: false } => {
                format!("{site} needs you signed in. Send the page from the Scrappy extension to share your sign-in.")
            }
            Self::NeedsLogin { site, tried: true } => {
                format!("{site} still needs a sign-in. Sign in there in your browser, then send the page again.")
            }
            Self::Failed(message) => message.clone(),
        }
    }
}

impl Downloads {
    /// No downloads yet; tools come from `toolbox`, remembered sign-ins from `vault`.
    pub fn new(toolbox: Toolbox, vault: Vault) -> Self {
        Self {
            probes: Probes::new(toolbox.clone(), vault),
            toolbox,
            queue: Queue::new(DownloadLimits::default().at_once as usize),
            entries: Arc::default(),
            list: DownloadList::default(),
            taken: Arc::default(),
        }
    }

    /// What's known about links: their probes, the link check, thumbnails and sizes.
    pub fn probes(&self) -> &Probes {
        &self.probes
    }

    /// Keeps the list in `dir` from now on, starting with what's kept there.
    pub fn kept_in(mut self, dir: &Path) -> Self {
        self.list = DownloadList::open(dir);
        self.entries = Arc::new(Mutex::new(self.list.load()));
        self
    }

    /// Starts with `limits` (the saved settings) instead of the defaults.
    pub fn limited(mut self, limits: &DownloadLimits) -> Self {
        self.queue = Queue::new(limits.at_once as usize);
        engine::set_speed_limit(limits.bytes_per_second());
        self
    }

    /// Runs downloads with new `limits`: the queue's size at once, the speed limit for downloads that start from
    /// now on. Must be called inside a Tokio runtime.
    pub fn set_limits(&self, limits: &DownloadLimits) {
        self.queue.resize(limits.at_once as usize);
        engine::set_speed_limit(limits.bytes_per_second());
    }

    /// The highest id in the list, so new jobs continue after it.
    pub fn last_id(&self) -> JobId {
        self.entries.lock().unwrap_or_else(PoisonError::into_inner).iter().map(|entry| entry.id).max().unwrap_or(0)
    }


    /// Whether a download is queued or running (a library move waits for none).
    pub fn busy(&self) -> bool {
        self.entries.lock().unwrap_or_else(PoisonError::into_inner).iter().any(|entry| !finished(entry))
    }

    /// The downloads, oldest first.
    pub fn list(&self) -> Vec<DownloadEntry> {
        self.entries.lock().unwrap_or_else(PoisonError::into_inner).clone()
    }

    /// Removes a finished download from the list (its files stay) and returns the list.
    pub fn remove(&self, id: JobId) -> Result<Vec<DownloadEntry>, String> {
        let mut entries = self.entries.lock().unwrap_or_else(PoisonError::into_inner);
        let index = entries.iter().position(|entry| entry.id == id).ok_or("That download isn't in the list.")?;
        if !finished(&entries[index]) {
            return Err("Stop the download first.".into());
        }
        entries.remove(index);
        self.list.remove(&[id]);
        Ok(entries.clone())
    }

    /// Removes every finished download from the list, or those from `source`, and returns what's left. Their files
    /// stay.
    pub fn clear(&self, source: Option<DownloadSource>) -> Vec<DownloadEntry> {
        let mut entries = self.entries.lock().unwrap_or_else(PoisonError::into_inner);
        let clears = |entry: &DownloadEntry| finished(entry) && source.is_none_or(|source| entry.source == source);
        let ids: Vec<JobId> = entries.iter().filter(|entry| clears(entry)).map(|entry| entry.id).collect();
        entries.retain(|entry| !clears(entry));
        self.list.remove(&ids);
        entries.clone()
    }

    /// Queues a download into the library at `root` (one per picked section) and returns the entries at once. Must
    /// be called inside a Tokio runtime.
    pub async fn start(
        &self,
        jobs: &Arc<Jobs>,
        root: Option<PathBuf>,
        request: DownloadRequest,
    ) -> Result<Vec<DownloadEntry>, String> {
        let root = root.ok_or("Set up the library first.")?;
        let preset = presets::find(&request.preset).ok_or("Unknown preset.")?;
        let choice = (preset, request.max_height, request.limit);
        let links = if request.sections.is_empty() { vec![request.url] } else { request.sections };
        let mut started = Vec::new();
        for link in links {
            let given = (Vec::new(), None, DownloadSource::Download, None);
            let entry = self.queue_link(jobs, root.clone(), &link, choice, &request.skip, given).await;
            started.push(entry.map_err(|e| e.message())?);
        }
        Ok(started)
    }

    /// Queues a link the extension sent, with the start preset and quality from Settings (an audio preset for a link
    /// with no video, ADR-0035), the page's allowed headers and a sign-in given for it, if any (`bridge` checked them
    /// all). A playlist or profile saves its first 50 files. A plain stream takes the page's title and site, and a
    /// preset `sent` names replaces the start one.
    pub async fn send_video(
        &self,
        jobs: &Arc<Jobs>,
        root: PathBuf,
        url: &str,
        sent: Sent,
        headers: Vec<(String, String)>,
        cookies: Option<Vec<Cookie>>,
    ) -> Result<DownloadEntry, SendError> {
        let checked = engine::check_url(url)?;
        let known = cookies.clone().or_else(|| self.probes.remembered(&checked));
        // Read first to know whether it has video; `queue_link` gets the same answer from the probe cache.
        let probed = self.probes.look_up(&checked, &headers, known.as_deref()).await?;
        let media = engine::options();
        // A list is video; a single link is audio only when every format it offers says so.
        let has_video = probed.probe.count.is_some() || !probe::Info::parse(&probed.json).is_ok_and(|info| info.audio_only());
        let preset = match sent.preset.as_deref() {
            Some(id) => presets::find(id).ok_or_else(|| SendError::Failed("Unknown preset.".into()))?,
            None => presets::for_send(media.preset.as_deref(), has_video),
        };
        let choice = (preset, media.max_height, Some(EXTENSION_LIMIT));
        let given = (headers, cookies, DownloadSource::Extension, Some(&sent));
        self.queue_link(jobs, root, url, choice, &[], given).await
    }

    /// Queues an image the extension sent; it's saved under `Images/{site}/`, the site taken from `page_url`.
    pub fn send_image(
        &self,
        jobs: &Arc<Jobs>,
        root: PathBuf,
        url: &str,
        page_url: Option<String>,
        title: Option<String>,
        headers: Vec<(String, String)>,
    ) -> Result<DownloadEntry, String> {
        let url = engine::check_url(url)?;
        let taken = self.taken.clone();
        let order = FileOrder {
            root,
            url: url.clone(),
            page_url,
            headers,
            taken,
            kind: "image",
            folder: None,
            min_bytes: 0,
            quiet: false,
        };
        let title = title.unwrap_or_else(|| url.clone());
        Ok(self.queue_file(jobs, row(DownloadSource::Extension, url, title, IMAGE_PRESET, None), order))
    }

    /// Saves one file now, inside `job` (a crawl, which reports its own progress), with the page as Referer.
    /// `None` when the file is under `min_bytes`; nothing of it is kept then.
    pub async fn save_now(
        &self,
        root: PathBuf,
        request: SaveRequest,
        min_bytes: u64,
        job: &Job,
    ) -> Result<Option<String>, String> {
        let url = engine::check_url(&request.url)?;
        let page = engine::check_url(&request.page_url)?;
        let headers = vec![("Referer".to_string(), page.clone())];
        let (taken, folder, kind) = (self.taken.clone(), Some(request.folder), request.kind);
        let order =
            FileOrder { root, url, page_url: Some(page), headers, taken, kind, folder, min_bytes, quiet: true };
        match file::run(order, job).await {
            Ok(path) => Ok(Some(path)),
            Err(error) if error == TOO_SMALL => Ok(None),
            Err(error) => Err(error),
        }
    }

    /// Queues a link Grab picked: a file fetched as it is, or a stream through yt-dlp. Streams are looked up first,
    /// so a link yt-dlp can't read fails here.
    pub async fn save(&self, jobs: &Arc<Jobs>, root: PathBuf, request: SaveRequest) -> Result<DownloadEntry, String> {
        let url = engine::check_url(&request.url)?;
        let page = engine::check_url(&request.page_url)?;
        let headers = vec![("Referer".to_string(), page.clone())];
        let taken = self.taken.clone();
        if !request.stream {
            let folder = Some(request.folder);
            let page_url = Some(page);
            let (kind, min_bytes, quiet) = (request.kind, 0, false);
            let order = FileOrder { root, url: url.clone(), page_url, headers, taken, kind, folder, min_bytes, quiet };
            return Ok(self.queue_file(jobs, row(DownloadSource::Grab, url, request.title, FILE_PRESET, None), order));
        }
        let info_json = probe::fetch(&self.toolbox, &url, &headers, None, false).await?;
        let preset = &presets::PRESETS[0];
        let name = template::render("{title}", &Fields { title: Some(request.title.clone()), ..Fields::default() });
        let (place, site) = (Some(request.folder.join(name)), request.site);
        let order = Order { root, url: url.clone(), preset, max_height: None, info_json, headers, cookies: None, taken, place, site };
        let toolbox = self.toolbox.clone();
        let row = row(DownloadSource::Grab, url, request.title, preset.name, None);
        let folder = download::folder(&order);
        let entry = self.queue(jobs, row, true, move |job| async move { download::run(&toolbox, order, &job).await });
        Ok(self.going_to(entry, folder))
    }

    /// Queues a link as the job its probe calls for: one video, a playlist or channel (yt-dlp), or a post or
    /// profile (gallery-dl). The preset, highest height and file limit come in `choice`, a playlist's videos to
    /// leave out in `skip`; the page's headers come with the sign-in `given` (else the site's remembered one is used),
    /// the row's source, and the page a plain stream is named after.
    async fn queue_link(
        &self,
        jobs: &Arc<Jobs>,
        root: PathBuf,
        url: &str,
        (preset, max_height, limit): (&'static Preset, Option<u32>, Option<u32>),
        skip: &[String],
        (headers, given, source, from): Given<'_>,
    ) -> Result<DownloadEntry, SendError> {
        let url = engine::check_url(url)?;
        let cookies = given.or_else(|| self.probes.remembered(&url));
        let Probed { json: info_json, probe, whole, .. } = self.probes.look_up(&url, &headers, cookies.as_deref()).await?;
        let toolbox = self.toolbox.clone();
        let tried = cookies.is_some();
        let link = url.clone();
        let fail = move |message: String| classify(&link, message, tried).message();
        let (title, thumbnail) = (probe.title, probe.thumbnail);
        if probe.files {
            let total = probe.count.filter(|_| !probe.more);
            let total = match (total, limit) {
                (Some(total), Some(limit)) => Some(total.min(limit)),
                (total, limit) => total.or(limit),
            };
            let order = GalleryOrder { root, url: url.clone(), limit, total, cookies };
            let row = row(source, url, title, FILES_PRESET, thumbnail);
            return Ok(self.queue(jobs, row, true, move |job| async move {
                gallery::run(&toolbox, order, &job).await.map_err(fail)
            }));
        }
        if probe.count.is_some() {
            // Video by video when the listing has them all, or some were left out (then only the listed ones go).
            let left_out = probe.entries.iter().any(|entry| skip.contains(&entry.url));
            let links = (!probe.entries.is_empty() && ((whole && !probe.more) || left_out)).then(|| {
                probe.entries.iter().filter(|entry| !skip.contains(&entry.url)).map(|entry| entry.url.clone()).collect()
            });
            let slots = self.queue.clone();
            let order =
                PlaylistOrder { root, url: url.clone(), preset, max_height, limit, links, info_json, cookies, slots };
            // Not in a slot itself: its videos take them.
            let row = row(source, url, title, preset.name, thumbnail);
            return Ok(self.queue(jobs, row, false, move |job| async move {
                playlist::run(&toolbox, order, &job).await.map_err(fail)
            }));
        }
        let taken = self.taken.clone();
        let named = from.and_then(|from| named(from, &info_json, preset.audio));
        let (title, place, site) = match named {
            Some((name, place, site)) => (name, Some(place), site),
            None => (title, None, None),
        };
        let order = Order { root, url: url.clone(), preset, max_height, info_json, headers, cookies, taken, place, site };
        let row = row(source, url, title, preset.name, thumbnail);
        let folder = download::folder(&order);
        let entry =
            self.queue(jobs, row, true, move |job| async move { download::run(&toolbox, order, &job).await.map_err(fail) });
        Ok(self.going_to(entry, folder))
    }


    /// Adds a row for a job another module runs (a scrape or a hunt) and starts it, outside the download slots.
    /// `work` returns how many files it saved into `folder` (relative to the library root). Must be called inside a
    /// Tokio runtime.
    pub fn track<F, Fut>(&self, jobs: &Arc<Jobs>, row: Row, folder: String, work: F) -> JobId
    where
        F: FnOnce(Job) -> Fut + Send + 'static,
        Fut: Future<Output = Result<u64, String>> + Send + 'static,
    {
        let entry = self.queue(jobs, row, false, move |job| async move {
            let files = u32::try_from(work(job).await?).unwrap_or(u32::MAX);
            let (path, files) = (Some(folder), Some(files));
            Ok(Saved { path, files, skipped: None, bytes: None, types: Vec::new(), warning: None })
        });
        entry.id
    }


    /// Queues a file fetched as it is.
    fn queue_file(&self, jobs: &Arc<Jobs>, row: Row, order: FileOrder) -> DownloadEntry {
        self.queue(jobs, row, true, move |job| async move {
            let root = order.root.clone();
            let path = file::run(order, &job).await?;
            let file = root.join(&path);
            let (bytes, types) = (size_of(&file), types_of(&[file]));
            Ok(Saved { path: Some(path), files: None, skipped: None, bytes, types, warning: None })
        })
    }

    /// Adds an entry and starts its job, in a download slot when `slotted`; the job's result becomes the entry's state.
    fn queue<F, Fut>(&self, jobs: &Arc<Jobs>, row: Row, slotted: bool, work: F) -> DownloadEntry
    where
        F: FnOnce(Job) -> Fut + Send + 'static,
        Fut: Future<Output = Result<Saved, String>> + Send + 'static,
    {
        // Held while the job starts, so its first update can't come before its entry exists.
        let mut entries = self.entries.lock().unwrap_or_else(PoisonError::into_inner);
        let downloads = self.clone();
        let run = move |job: Job| async move {
            let id = job.id();
            downloads.update(id, |entry| entry.state = DownloadState::Running);
            let cancel = job.clone();
            let result = work(job).await;
            downloads.update(id, |entry| match &result {
                _ if cancel.cancelled() => entry.state = DownloadState::Cancelled,
                Ok(saved) => {
                    entry.state = DownloadState::Done;
                    (entry.file, entry.files, entry.skipped) = (saved.path.clone(), saved.files, saved.skipped);
                    entry.size = saved.bytes;
                    entry.types.clone_from(&saved.types);
                    entry.error.clone_from(&saved.warning);
                }
                Err(message) => (entry.state, entry.error) = (DownloadState::Failed, Some(message.clone())),
            });
            result.map(|_| ())
        };
        let id = if slotted { jobs.start_in(&self.queue, run) } else { jobs.start(run) };
        let entry = DownloadEntry {
            id,
            source: row.source,
            url: row.url,
            title: row.title,
            preset: row.kind,
            state: DownloadState::Queued,
            thumbnail: row.thumbnail,
            file: None,
            files: None,
            skipped: None,
            size: None,
            error: None,
            types: Vec::new(),
        };
        self.list.put(&entry);
        entries.push(entry.clone());
        entry
    }

    /// Notes the folder `entry` saves into, so a failed download can show it; the saved file replaces it when it's
    /// done.
    fn going_to(&self, mut entry: DownloadEntry, folder: Option<String>) -> DownloadEntry {
        if let Some(folder) = folder {
            self.update(entry.id, |row| {
                row.file.get_or_insert_with(|| folder.clone());
            });
            entry.file.get_or_insert(folder);
        }
        entry
    }

    fn update(&self, id: JobId, change: impl FnOnce(&mut DownloadEntry)) {
        let mut entries = self.entries.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(entry) = entries.iter_mut().find(|entry| entry.id == id) {
            change(entry);
            self.list.put(entry);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{DownloadRequest, DownloadSource, DownloadState, Downloads, Row, Sent, named};
    use crate::jobs::{JobEvent, Jobs};
    use crate::tools::Toolbox;
    use crate::vault::{KeySource, Vault};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    #[test]
    fn names_a_plain_stream_after_its_page() {
        let page_url = Some("https://www.pornhub.com/view_video.php?viewkey=a".to_string());
        let from = Sent { page_url, title: Some("Clip".into()), preset: None };
        let stream = r#"{"id":"master","title":"master","extractor_key":"Generic","webpage_url":"https://cdn.example/master.m3u8"}"#;
        let (title, place, site) = named(&from, stream, false).unwrap();
        assert_eq!((title.as_str(), site.as_deref()), ("Clip", Some("pornhub.com")));
        assert!(place.starts_with("Videos/pornhub.com") && place.ends_with("Clip"), "{place:?}");
        // A site yt-dlp reads keeps yt-dlp's own names.
        let known = r#"{"id":"a","title":"Real title","extractor_key":"PornHub","webpage_url":"https://www.pornhub.com/view_video.php?viewkey=a"}"#;
        assert!(named(&from, known, false).is_none());
    }

    async fn wait_for(finished: &Mutex<usize>, count: usize) {
        while *finished.lock().unwrap() < count {
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    }

    /// Jobs that count finished ones, and a toolbox with every tool: from `SCRAPPY_TOOLS` when set (so runs can
    /// share one download), else fetched into `dir`.
    async fn setup(dir: &std::path::Path) -> (Arc<Jobs>, Arc<Mutex<usize>>, Toolbox, usize) {
        let finished = Arc::new(Mutex::new(0));
        let sink = finished.clone();
        let jobs = Jobs::new(Arc::new(move |event| {
            if let JobEvent::Finished(end) = event {
                println!("{end:?}");
                *sink.lock().unwrap() += 1;
            }
        }));
        let tools = std::env::var("SCRAPPY_TOOLS").map_or_else(|_| dir.join("tools"), Into::into);
        let toolbox = Toolbox::new(tools);
        let installs = toolbox.install_needed(&jobs).await.len();
        wait_for(&finished, installs).await;
        (jobs, finished, toolbox, installs)
    }

    #[tokio::test]
    async fn tracks_other_jobs_and_clears_by_source() {
        let dir = tempfile::tempdir().unwrap();
        let finished = Arc::new(Mutex::new(0));
        let sink = finished.clone();
        let jobs = Jobs::new(Arc::new(move |event| {
            if let JobEvent::Finished(_) = event {
                *sink.lock().unwrap() += 1;
            }
        }));
        let vault = Vault::open(&dir.path().join("vault"), KeySource::Fixed([1; 32]));
        let downloads = Downloads::new(Toolbox::new(dir.path().join("tools")), vault);
        for (source, kind) in [(DownloadSource::Scrape, "Mirror"), (DownloadSource::Hunt, "Images")] {
            let row = Row { source, url: String::new(), title: "cats".into(), kind: kind.into(), thumbnail: None };
            downloads.track(&jobs, row, "Sites/example.com".into(), |_| async { Ok(3) });
        }
        wait_for(&finished, 2).await;
        let done: Vec<_> = downloads.list().into_iter().map(|e| (e.source, e.state, e.files, e.file)).collect();
        let folder = Some("Sites/example.com".to_string());
        assert_eq!(done[0], (DownloadSource::Scrape, DownloadState::Done, Some(3), folder));
        let left = downloads.clear(Some(DownloadSource::Scrape));
        assert_eq!(left.iter().map(|e| e.source).collect::<Vec<_>>(), [DownloadSource::Hunt]);
        assert!(downloads.clear(None).is_empty());
    }

    /// Proves the whole pipeline on a 19-second public video: run with `cargo test -p scrappy-core -- --ignored`.
    #[tokio::test]
    #[ignore = "downloads the tools and a YouTube video"]
    async fn downloads_a_video_three_ways() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("Library");
        std::fs::create_dir_all(root.join(".scrappy")).unwrap();
        let (jobs, finished, toolbox, installs) = setup(dir.path()).await;

        let vault = Vault::open(&dir.path().join("vault"), KeySource::Fixed([1; 32]));
        let downloads = Downloads::new(toolbox, vault);
        let url = "https://www.youtube.com/watch?v=jNQXAC9IVRw";
        let probe = downloads.probes.look_up(url, &[], None).await.unwrap().probe;
        assert_eq!(probe.title, "Me at the zoo");
        assert!(probe.thumbnail.as_deref().is_some_and(|t| t.starts_with("data:image/")));
        for preset in ["editor-mp4", "hevc-mp4", "mp3"] {
            let request = DownloadRequest { url: url.into(), preset: preset.into(), max_height: Some(240), limit: None, sections: Vec::new(), skip: Vec::new() };
            downloads.start(&jobs, Some(root.clone()), request).await.unwrap();
        }
        wait_for(&finished, installs + 3).await;

        for entry in downloads.list() {
            assert_eq!(entry.state, DownloadState::Done, "{entry:?}");
            assert!(root.join(entry.file.unwrap()).is_file());
        }
        let rows: i64 = rusqlite::Connection::open(root.join(".scrappy/library.db"))
            .unwrap()
            .query_row("SELECT COUNT(*) FROM downloads", [], |row| row.get(0))
            .unwrap();
        assert_eq!(rows, 3);
    }


    /// Proves collections: two videos of a playlist, then two files of a Bluesky profile, each twice so the second
    /// run skips them. Run with `cargo test -p scrappy-core -- --ignored`.
    #[tokio::test]
    #[ignore = "downloads the tools, two YouTube videos and two Bluesky files"]
    async fn downloads_a_playlist_and_a_profile() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("Library");
        std::fs::create_dir_all(root.join(".scrappy")).unwrap();
        let (jobs, finished, toolbox, installs) = setup(dir.path()).await;
        let vault = Vault::open(&dir.path().join("vault"), KeySource::Fixed([1; 32]));
        let downloads = Downloads::new(toolbox, vault);

        let links = [
            ("https://www.youtube.com/playlist?list=PLzMcBGfZo4-lp3jAExUCewBfMx3UZFkh5", "mp3"),
            ("https://bsky.app/profile/bsky.app", "editor-mp4"),
        ];
        let mut done = installs;
        for (url, preset) in links {
            let probe = downloads.probes.look_up(url, &[], None).await.unwrap().probe;
            assert!(probe.count.is_some_and(|count| count >= 2), "{probe:?}");
            for _ in 0..2 {
                let request = DownloadRequest { url: url.into(), preset: preset.into(), max_height: None, limit: Some(2), sections: Vec::new(), skip: Vec::new() };
                downloads.start(&jobs, Some(root.clone()), request).await.unwrap();
                done += 1;
                wait_for(&finished, done).await;
            }
        }

        let entries = downloads.list();
        let counts: Vec<_> = entries.iter().map(|entry| (entry.state, entry.files, entry.skipped)).collect();
        let first = (DownloadState::Done, Some(2), Some(0));
        let again = (DownloadState::Done, Some(0), Some(2));
        assert_eq!(counts, [first, again, first, again], "{entries:?}");
        assert!(entries.iter().all(|entry| root.join(entry.file.as_deref().unwrap()).is_dir()));
        let kinds: Vec<(String, i64)> = rusqlite::Connection::open(root.join(".scrappy/library.db"))
            .unwrap()
            .prepare("SELECT kind, COUNT(*) FROM downloads GROUP BY kind ORDER BY kind")
            .unwrap()
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(kinds, [("audio".to_string(), 2), ("social".to_string(), 2)]);
    }
}

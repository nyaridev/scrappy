//! Downloads: the download list (kept on disk when the app asks, ADR-0016), the links probed so far (so a download
//! reuses the probe instead of asking the site again), the queue downloads wait in (ADR-0010), and the sign-ins they
//! use: one given for this download, else the site's remembered one (ADR-0005). A link becomes one video, a playlist
//! or a gallery-dl collection, as its route and probe say; a channel's picked sections become one download each.
//! The queue's size and the speed limit come from the settings (ADR-0017).

// ── Imports ───────────────────────────────
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::download::{self, Order, Saved, Taken, size_of};
use super::download_list::DownloadList;
use super::engine;
use super::gallery::{self, GalleryOrder};
use super::file::{self, FileOrder, TOO_SMALL};
use super::playlist::{self, PlaylistOrder};
use super::presets::{self, Preset};
use super::probe::{self, Info, Probe};
use super::route::{self, Engine};
use crate::jobs::{Job, JobId, Jobs, Queue};
use crate::library::template::{self, Fields};
use crate::settings::DownloadLimits;
use crate::tools::Toolbox;
use crate::vault::{self, Cookie, CookieFile, Vault};

// ── Constants ─────────────────────────────
/// The preset column for an image sent from the extension.
const IMAGE_PRESET: &str = "Image";
/// The preset column for a post or profile: gallery-dl saves the files as they are.
const FILES_PRESET: &str = "Original files";
/// The preset column for a file Grab picked, saved as it is.
const FILE_PRESET: &str = "Original file";
/// How many files of a playlist or profile a send from the extension saves.
const EXTENSION_LIMIT: u32 = 50;
/// How many listed videos' thumbnails are fetched at once.
const PREVIEWS_AT_ONCE: usize = 6;
/// A probe's stream links expire after some hours; an older probe is done again.
const PROBE_KEPT: Duration = Duration::from_secs(60 * 60);

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

/// One download in the list. Progress arrives as job events with the same id.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DownloadEntry {
    pub id: JobId,
    pub url: String,
    pub title: String,
    /// The preset's name.
    pub preset: String,
    pub state: DownloadState,
    /// A small `data:` URL from the probe.
    pub thumbnail: Option<String>,
    /// The saved file, or the folder a playlist or profile went to, relative to the library root.
    pub file: Option<String>,
    /// For a playlist or profile: the files saved.
    pub files: Option<u32>,
    /// For a playlist or profile: the files skipped because they were saved before.
    pub skipped: Option<u32>,
    /// Bytes on disk of what it saved; `None` until it's done, and for rows kept from before sizes were.
    #[ts(type = "number | null")]
    pub size: Option<u64>,
    pub error: Option<String>,
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

/// A size estimate's link, preset id and height.
type SizeKey = (String, &'static str, Option<u32>);

/// A link's probe, kept for its download.
#[derive(Clone)]
struct Probed {
    at: Instant,
    /// yt-dlp's JSON; empty for gallery-dl's links.
    json: String,
    probe: Probe,
}

/// The downloads and what they share.
#[derive(Clone)]
pub struct Downloads {
    toolbox: Toolbox,
    vault: Vault,
    queue: Queue,
    entries: Arc<Mutex<Vec<DownloadEntry>>>,
    list: DownloadList,
    probed: Arc<Mutex<HashMap<String, Probed>>>,
    /// Listed videos' thumbnail links, by video link, and the pictures fetched so far.
    previews: Arc<Mutex<HashMap<String, String>>>,
    pictures: Arc<Mutex<HashMap<String, Option<String>>>>,
    fetching: Arc<tokio::sync::Semaphore>,
    sizes: Arc<Mutex<HashMap<SizeKey, Option<u64>>>>,
    taken: Taken,
}

// ── Private helpers ───────────────────────
/// Done, failed or stopped: it can leave the list.
fn finished(entry: &DownloadEntry) -> bool {
    !matches!(entry.state, DownloadState::Queued | DownloadState::Running)
}

impl From<String> for SendError {
    fn from(message: String) -> Self {
        Self::Failed(message)
    }
}

/// A tool's error for `url`, as a sign-in wall when it reads like one, or when the site hides things from visitors
/// who aren't signed in and answered "not found".
fn classify(url: &str, message: String, tried: bool) -> SendError {
    let lower = message.to_lowercase();
    let hidden = route::hides_without_sign_in(url) && (lower.contains("not found") || lower.contains("could not be found"));
    match vault::site_of(url) {
        Some(site) if hidden || engine::needs_login(&message) => SendError::NeedsLogin { site, tried },
        _ => SendError::Failed(message),
    }
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
            toolbox,
            vault,
            queue: Queue::new(DownloadLimits::default().at_once as usize),
            entries: Arc::default(),
            list: DownloadList::default(),
            probed: Arc::default(),
            previews: Arc::default(),
            pictures: Arc::default(),
            fetching: Arc::new(tokio::sync::Semaphore::new(PREVIEWS_AT_ONCE)),
            sizes: Arc::default(),
            taken: Arc::default(),
        }
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

    /// Reads a link's details for the window, and keeps them for its download. Uses the site's remembered sign-in.
    pub async fn probe(&self, url: &str) -> Result<Probe, String> {
        let url = engine::check_url(url)?;
        let cookies = self.remembered(&url);
        Ok(self.look_up(&url, &[], cookies.as_deref()).await.map_err(|e| e.message())?.probe)
    }

    /// The size a probed video will download with `preset` at `max_height`; `None` for a link that isn't one
    /// probed video, or when the site doesn't say.
    pub async fn size(&self, url: &str, preset: &str, max_height: Option<u32>) -> Result<Option<u64>, String> {
        let url = engine::check_url(url)?;
        let preset = presets::find(preset).ok_or("Unknown preset.")?;
        let key = (url.clone(), preset.id, max_height.filter(|_| !preset.audio));
        if let Some(size) = self.sizes.lock().unwrap_or_else(PoisonError::into_inner).get(&key) {
            return Ok(*size);
        }
        let kept = self.probed.lock().unwrap_or_else(PoisonError::into_inner).get(&url).cloned();
        let Some(Probed { json, probe, .. }) = kept else { return Ok(None) };
        if probe.files || probe.count.is_some() || json.is_empty() {
            return Ok(None);
        }
        let size = download::estimate(&self.toolbox, preset, key.2, &json).await?;
        self.sizes.lock().unwrap_or_else(PoisonError::into_inner).insert(key, size);
        Ok(size)
    }

    /// The thumbnail of a video a probe listed, as a `data:` URL; `None` for a link no probe listed, or no picture.
    pub async fn preview(&self, url: &str) -> Option<String> {
        if let Some(picture) = self.pictures.lock().unwrap_or_else(PoisonError::into_inner).get(url) {
            return picture.clone();
        }
        let link = self.previews.lock().unwrap_or_else(PoisonError::into_inner).get(url).cloned()?;
        let _turn = self.fetching.acquire().await.ok()?;
        let picture = probe::thumbnail(Some(&link)).await;
        self.pictures.lock().unwrap_or_else(PoisonError::into_inner).insert(url.to_string(), picture.clone());
        picture
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

    /// Removes every finished download from the list (their files stay) and returns what's left.
    pub fn clear(&self) -> Vec<DownloadEntry> {
        let mut entries = self.entries.lock().unwrap_or_else(PoisonError::into_inner);
        let ids: Vec<JobId> = entries.iter().filter(|entry| finished(entry)).map(|entry| entry.id).collect();
        entries.retain(|entry| !finished(entry));
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
            let entry = self.queue_link(jobs, root.clone(), &link, choice, &request.skip, (Vec::new(), None)).await;
            started.push(entry.map_err(|e| e.message())?);
        }
        Ok(started)
    }

    /// Queues a link the extension sent, with the default preset, the page's allowed headers and a sign-in given
    /// for it, if any (`bridge` checked them all). A playlist or profile saves its first 50 files.
    pub async fn send_video(
        &self,
        jobs: &Arc<Jobs>,
        root: PathBuf,
        url: &str,
        headers: Vec<(String, String)>,
        cookies: Option<Vec<Cookie>>,
    ) -> Result<DownloadEntry, SendError> {
        let choice = (&presets::PRESETS[0], None, Some(EXTENSION_LIMIT));
        self.queue_link(jobs, root, url, choice, &[], (headers, cookies)).await
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
        Ok(self.queue_file(jobs, url, title, IMAGE_PRESET, order))
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
            return Ok(self.queue_file(jobs, url, request.title, FILE_PRESET, order));
        }
        let info_json = probe::fetch(&self.toolbox, &url, &headers, None).await?;
        let preset = &presets::PRESETS[0];
        let name = template::render("{title}", &Fields { title: Some(request.title.clone()), ..Fields::default() });
        let (place, site) = (Some(request.folder.join(name)), request.site);
        let order = Order { root, url: url.clone(), preset, max_height: None, info_json, headers, cookies: None, taken, place, site };
        let toolbox = self.toolbox.clone();
        Ok(self.queue(jobs, (url, request.title), preset.name, None, true, move |job| async move {
            download::run(&toolbox, order, &job).await
        }))
    }

    /// Queues a link as the job its probe calls for: one video, a playlist or channel (yt-dlp), or a post or
    /// profile (gallery-dl). The preset, highest height and file limit come in `choice`, a playlist's videos to
    /// leave out in `skip`; the page's headers come with the sign-in `given`, else the site's remembered one is used.
    async fn queue_link(
        &self,
        jobs: &Arc<Jobs>,
        root: PathBuf,
        url: &str,
        (preset, max_height, limit): (&'static Preset, Option<u32>, Option<u32>),
        skip: &[String],
        (headers, given): (Vec<(String, String)>, Option<Vec<Cookie>>),
    ) -> Result<DownloadEntry, SendError> {
        let url = engine::check_url(url)?;
        let cookies = given.or_else(|| self.remembered(&url));
        let Probed { json: info_json, probe, .. } = self.look_up(&url, &headers, cookies.as_deref()).await?;
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
            return Ok(self.queue(jobs, (url, title), FILES_PRESET, thumbnail, true, move |job| async move {
                gallery::run(&toolbox, order, &job).await.map_err(fail)
            }));
        }
        if probe.count.is_some() {
            // Video by video when the listing has them all, or some were left out (then only the listed ones go).
            let left_out = probe.entries.iter().any(|entry| skip.contains(&entry.url));
            let links = (!probe.entries.is_empty() && (!probe.more || left_out)).then(|| {
                probe.entries.iter().filter(|entry| !skip.contains(&entry.url)).map(|entry| entry.url.clone()).collect()
            });
            let slots = self.queue.clone();
            let order =
                PlaylistOrder { root, url: url.clone(), preset, max_height, limit, links, info_json, cookies, slots };
            // Not in a slot itself: its videos take them.
            return Ok(self.queue(jobs, (url, title), preset.name, thumbnail, false, move |job| async move {
                playlist::run(&toolbox, order, &job).await.map_err(fail)
            }));
        }
        let taken = self.taken.clone();
        let order =
            Order { root, url: url.clone(), preset, max_height, info_json, headers, cookies, taken, place: None, site: None };
        Ok(self.queue(jobs, (url, title), preset.name, thumbnail, true, move |job| async move {
            download::run(&toolbox, order, &job).await.map_err(fail)
        }))
    }

    /// The site's remembered sign-in, if there is one.
    fn remembered(&self, url: &str) -> Option<Vec<Cookie>> {
        vault::site_of(url).and_then(|site| self.vault.cookies_for(&site))
    }

    /// `url`'s probe: kept from the last hour, else asked from the tool its route names, with a temporary cookies
    /// file while it runs when there's a sign-in.
    async fn look_up(
        &self,
        url: &str,
        headers: &[(String, String)],
        cookies: Option<&[Cookie]>,
    ) -> Result<Probed, SendError> {
        let kept = self.probed.lock().unwrap_or_else(PoisonError::into_inner).get(url).cloned();
        if let Some(kept) = kept.filter(|probed| probed.at.elapsed() < PROBE_KEPT) {
            return Ok(kept);
        }
        let tried = cookies.is_some();
        let file = cookies.map(CookieFile::write).transpose()?;
        let (json, probe) = match route::route(url) {
            Engine::GalleryDl => {
                let listing = gallery::list(&self.toolbox, url, file.as_ref())
                    .await
                    .map_err(|message| classify(url, message, tried))?;
                if listing.count == 0 {
                    return Err(SendError::Failed("There's nothing to download here.".into()));
                }
                let thumbnail = probe::thumbnail(listing.preview.as_deref()).await;
                (String::new(), listing.probe(url, thumbnail))
            }
            Engine::YtDlp => {
                let json = probe::fetch(&self.toolbox, url, headers, file.as_ref())
                    .await
                    .map_err(|message| classify(url, message, tried))?;
                let info = Info::parse(&json)?;
                self.previews.lock().unwrap_or_else(PoisonError::into_inner).extend(info.previews());
                let thumbnail = probe::thumbnail(info.thumbnail_url()).await;
                // A channel's sections come listed inside its listing: kept, so each downloads without a new probe.
                for (link, json) in probe::section_listings(&json) {
                    let probe = Info::parse(&json)?.probe(&link, thumbnail.clone());
                    let probed = Probed { at: Instant::now(), json, probe };
                    self.probed.lock().unwrap_or_else(PoisonError::into_inner).insert(link, probed);
                }
                let probe = info.probe(url, thumbnail);
                (json, probe)
            }
        };
        let probed = Probed { at: Instant::now(), json, probe };
        self.probed.lock().unwrap_or_else(PoisonError::into_inner).insert(url.to_string(), probed.clone());
        Ok(probed)
    }

    /// Queues a file fetched as it is.
    fn queue_file(&self, jobs: &Arc<Jobs>, url: String, title: String, preset: &str, order: FileOrder) -> DownloadEntry {
        self.queue(jobs, (url, title), preset, None, true, move |job| async move {
            let root = order.root.clone();
            let path = file::run(order, &job).await?;
            let bytes = size_of(&root.join(&path));
            Ok(Saved { path: Some(path), files: None, skipped: None, bytes })
        })
    }

    /// Adds an entry and starts its job, in a download slot when `slotted`; the job's result becomes the entry's state.
    fn queue<F, Fut>(
        &self,
        jobs: &Arc<Jobs>,
        (url, title): (String, String),
        preset: &str,
        thumbnail: Option<String>,
        slotted: bool,
        work: F,
    ) -> DownloadEntry
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
                }
                Err(message) => (entry.state, entry.error) = (DownloadState::Failed, Some(message.clone())),
            });
            result.map(|_| ())
        };
        let id = if slotted { jobs.start_in(&self.queue, run) } else { jobs.start(run) };
        let entry = DownloadEntry {
            id,
            url,
            title,
            preset: preset.to_string(),
            state: DownloadState::Queued,
            thumbnail,
            file: None,
            files: None,
            skipped: None,
            size: None,
            error: None,
        };
        self.list.put(&entry);
        entries.push(entry.clone());
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
    use super::{DownloadRequest, DownloadState, Downloads, SendError, classify};
    use crate::jobs::{JobEvent, Jobs};
    use crate::tools::Toolbox;
    use crate::vault::{KeySource, Vault};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

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

    #[test]
    fn reads_not_found_as_a_sign_in_where_sites_hide_things() {
        let missing = || "Requested user could not be found".to_string();
        let instagram = classify("https://www.instagram.com/nasa/", missing(), false);
        assert_eq!(instagram, SendError::NeedsLogin { site: "instagram.com".into(), tried: false });
        assert_eq!(classify("https://bsky.app/profile/a", missing(), false), SendError::Failed(missing()));
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
        let probe = downloads.probe(url).await.unwrap();
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

    /// Proves a channel's sections are counted one by one, list their videos, and are kept for their downloads.
    /// Needs `SCRAPPY_TOOLS` (a folder with yt-dlp); run with `cargo test -p scrappy-core -- --ignored counts_a_channels`.
    #[tokio::test]
    #[ignore = "asks YouTube for a channel's listing"]
    async fn counts_a_channels_videos_and_shorts() {
        let dir = tempfile::tempdir().unwrap();
        let tools = std::env::var("SCRAPPY_TOOLS").expect("SCRAPPY_TOOLS");
        let vault = Vault::open(&dir.path().join("vault"), KeySource::Fixed([1; 32]));
        let downloads = Downloads::new(Toolbox::new(tools.into()), vault);
        let probe = downloads.probe("https://www.youtube.com/@GDULA7").await.unwrap();
        let counts: Vec<_> = probe.sections.iter().map(|s| (s.title.as_str(), s.count, s.entries.len())).collect();
        println!("{} {:?} {counts:?}", probe.title, probe.count);
        assert!(probe.sections.len() >= 2, "{probe:?}");
        assert_eq!(probe.count, Some(probe.sections.iter().map(|s| s.count).sum()));
        assert!(probe.sections.iter().all(|s| s.entries.len() == s.count as usize), "{counts:?}");
        let videos = &probe.sections[0];
        assert_eq!(videos.title, "Videos");
        let kept = downloads.probe(&videos.url).await.unwrap();
        assert_eq!((kept.count, kept.entries.len()), (Some(videos.count), videos.entries.len()));
        let short = &probe.sections[1].entries[0];
        assert!(short.tall && !videos.entries[0].tall, "{short:?}");
        let picture = downloads.preview(&short.url).await.unwrap();
        assert!(picture.starts_with("data:image/"));
        assert_eq!(downloads.preview("https://example.com/not-listed").await, None);
    }

    /// Proves a video link that names a playlist probes as the playlist. Needs `SCRAPPY_TOOLS`, like the test above.
    #[tokio::test]
    #[ignore = "asks YouTube for a playlist's listing"]
    async fn reads_a_video_in_a_playlist_as_the_playlist() {
        let dir = tempfile::tempdir().unwrap();
        let tools = std::env::var("SCRAPPY_TOOLS").expect("SCRAPPY_TOOLS");
        let vault = Vault::open(&dir.path().join("vault"), KeySource::Fixed([1; 32]));
        let downloads = Downloads::new(Toolbox::new(tools.into()), vault);
        let url = "https://www.youtube.com/watch?v=cvaIgq5j2Q8&list=PL0wqt_um4x0bsdViTJBmnl6KGMoSqxfZy";
        let probe = downloads.probe(url).await.unwrap();
        println!("{} {:?} {}", probe.title, probe.count, probe.entries.len());
        assert!(probe.count.is_some_and(|count| count > 100) && !probe.entries.is_empty(), "{probe:?}");
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
            let probe = downloads.probe(url).await.unwrap();
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

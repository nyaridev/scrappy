//! Probes: what Scrappy knows about links before they download. Each link's probe is kept for an hour, so its
//! download doesn't ask the site again (ADR-0016); the window's link check is a job that shows a list as it's listed
//! (ADR-0030); listed videos' thumbnails and a video's size per preset are fetched when the window asks. Probes use
//! the site's remembered sign-in (ADR-0005).

// ── Imports ───────────────────────────────
use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use super::download;
use super::downloads::SendError;
use super::engine;
use super::gallery;
use super::listing::{self, Check, State};
use super::presets;
use super::probe::{self, Info, Probe};
use super::route::{self, Engine};
use super::thumbnail;
use crate::jobs::{Job, JobId, Jobs};
use crate::tools::Toolbox;
use crate::vault::{self, Cookie, CookieFile, Vault};

// ── Constants ─────────────────────────────
/// How many listed videos' thumbnails are fetched at once.
const PREVIEWS_AT_ONCE: usize = 6;
/// A probe's stream links expire after some hours; an older probe is done again.
const PROBE_KEPT: Duration = Duration::from_secs(60 * 60);

// ── Types ─────────────────────────────────
/// A size estimate's link, preset id and height.
type SizeKey = (String, &'static str, Option<u32>);

/// A link's probe, kept for its download.
#[derive(Clone)]
pub(super) struct Probed {
    pub(super) at: Instant,
    /// yt-dlp's JSON; empty for gallery-dl's links.
    pub(super) json: String,
    pub(super) probe: Probe,
    /// Its listing ended: a check still listing gives a part of the list (ADR-0030).
    pub(super) whole: bool,
}

/// The probes, the link check and the thumbnails fetched so far. Clones share them.
#[derive(Clone)]
pub struct Probes {
    toolbox: Toolbox,
    vault: Vault,
    probed: Arc<Mutex<HashMap<String, Probed>>>,
    /// The latest link check (ADR-0030).
    check: Arc<Mutex<Option<Check>>>,
    /// Listed videos' thumbnail links, by video link, and the pictures fetched so far.
    previews: Arc<Mutex<HashMap<String, String>>>,
    pictures: Arc<Mutex<HashMap<String, Option<String>>>>,
    fetching: Arc<tokio::sync::Semaphore>,
    sizes: Arc<Mutex<HashMap<SizeKey, Option<u64>>>>,
}

// ── Private helpers ───────────────────────
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

// ── Public API ────────────────────────────
/// A tool's error for `url`, as a sign-in wall when it reads like one, or when the site hides things from visitors
/// who aren't signed in and answered "not found".
pub(super) fn classify(url: &str, message: String, tried: bool) -> SendError {
    let lower = message.to_lowercase();
    let hidden = route::hides_without_sign_in(url) && (lower.contains("not found") || lower.contains("could not be found"));
    match vault::site_of(url) {
        Some(site) if hidden || engine::needs_login(&message) => SendError::NeedsLogin { site, tried },
        _ => SendError::Failed(message),
    }
}

impl Probes {
    /// Nothing known yet; tools come from `toolbox`, remembered sign-ins from `vault`.
    pub(super) fn new(toolbox: Toolbox, vault: Vault) -> Self {
        Self {
            toolbox,
            vault,
            probed: Arc::default(),
            check: Arc::default(),
            previews: Arc::default(),
            pictures: Arc::default(),
            fetching: Arc::new(tokio::sync::Semaphore::new(PREVIEWS_AT_ONCE)),
            sizes: Arc::default(),
        }
    }

    /// Checks a link for the window as a job, replacing the check before (ADR-0030): a list shows as it's listed, its
    /// progress the videos listed so far, and `listed_link` returns it. What it found is kept for its download. Uses
    /// the site's remembered sign-in. Must be called inside a Tokio runtime.
    pub fn list_link(&self, jobs: &Arc<Jobs>, url: &str) -> Result<JobId, String> {
        let url = engine::check_url(url)?;
        let probes = self.clone();
        Ok(jobs.start(move |job| async move {
            let (id, listing) = (job.id(), listing::Listing::default());
            let check = Check { id, url: url.clone(), listing, state: State::Listing, thumbnail: None, probe: None };
            *lock(&probes.check) = Some(check);
            let probe = probes.check_now(&url, &job).await.map_err(|e| e.message())?.probe;
            probes.in_check(id, |check| check.probe = Some(probe));
            Ok(())
        }))
    }

    /// What the check `id` has found so far; all of it once its job has ended. `None` before it lists anything, and
    /// once a newer check started.
    pub fn listed_link(&self, id: JobId) -> Option<Probe> {
        let check = lock(&self.check);
        let check = check.as_ref().filter(|check| check.id == id)?;
        if let Some(probe) = &check.probe {
            return Some(probe.clone());
        }
        let (_, info, probe) = check.build()?;
        lock(&self.previews).extend(info.previews());
        Some(probe)
    }

    /// The size a probed video will download with `preset` at `max_height`; `None` for a link that isn't one
    /// probed video, or when the site doesn't say.
    pub async fn size(&self, url: &str, preset: &str, max_height: Option<u32>) -> Result<Option<u64>, String> {
        let url = engine::check_url(url)?;
        let preset = presets::find(preset).ok_or("Unknown preset.")?;
        let key = (url.clone(), preset.id, max_height.filter(|_| !preset.audio));
        if let Some(size) = lock(&self.sizes).get(&key) {
            return Ok(*size);
        }
        let kept = lock(&self.probed).get(&url).cloned();
        let Some(Probed { json, probe, .. }) = kept else { return Ok(None) };
        if probe.files || probe.count.is_some() || json.is_empty() {
            return Ok(None);
        }
        let size = download::estimate(&self.toolbox, preset, key.2, &json).await?;
        lock(&self.sizes).insert(key, size);
        Ok(size)
    }

    /// The thumbnail of a video a probe listed, as a `data:` URL; `None` for a link no probe listed, or no picture.
    pub async fn preview(&self, url: &str) -> Option<String> {
        if let Some(picture) = lock(&self.pictures).get(url) {
            return picture.clone();
        }
        let link = lock(&self.previews).get(url).cloned()?;
        let _turn = self.fetching.acquire().await.ok()?;
        let picture = probe::thumbnail(Some(&link)).await;
        lock(&self.pictures).insert(url.to_string(), picture.clone());
        picture
    }

    /// The site's remembered sign-in, if there is one.
    pub(super) fn remembered(&self, url: &str) -> Option<Vec<Cookie>> {
        vault::site_of(url).and_then(|site| self.vault.cookies_for(&site))
    }

    /// `url`'s probe: kept from the last hour, else asked from the tool its route names, with a temporary cookies
    /// file while it runs when there's a sign-in.
    pub(super) async fn look_up(
        &self,
        url: &str,
        headers: &[(String, String)],
        cookies: Option<&[Cookie]>,
    ) -> Result<Probed, SendError> {
        let kept = lock(&self.probed).get(url).cloned();
        if let Some(kept) = kept.filter(|probed| probed.at.elapsed() < PROBE_KEPT) {
            return Ok(kept);
        }
        if let Some(listed) = self.listing_now(url) {
            return Ok(listed);
        }
        let tried = cookies.is_some();
        let file = cookies.map(CookieFile::write).transpose()?;
        let file = file.as_ref();
        let (json, probe) = match route::route(url) {
            Engine::GalleryDl => self.ask_gallery_dl(url, file, tried).await?,
            Engine::YtDlp => self.ask_yt_dlp(url, headers, file, tried, false).await?,
            // A post: its video through yt-dlp, with the qualities on offer; images (no video) through gallery-dl.
            Engine::YtDlpThenGalleryDl => match self.ask_yt_dlp(url, headers, file, tried, false).await {
                Ok(found) => found,
                Err(_) => self.ask_gallery_dl(url, file, tried).await?,
            },
        };
        Ok(self.keep(url, json, probe))
    }

    /// What `url` would download, for the extension's popup (ADR-0042): kept from the last hour, else read by the
    /// tool its route names with yt-dlp's generic reader off, and kept for its download. `None` when no site reader
    /// knows the page. Uses the site's remembered sign-in.
    pub async fn look(&self, url: &str) -> Result<Option<Probe>, SendError> {
        let url = &engine::check_url(url)?;
        let kept = lock(&self.probed).get(url).cloned();
        if let Some(kept) = kept.filter(|probed| probed.at.elapsed() < PROBE_KEPT) {
            return Ok(Some(kept.probe));
        }
        let cookies = self.remembered(url);
        let tried = cookies.is_some();
        let file = cookies.as_deref().map(CookieFile::write).transpose()?;
        let file = file.as_ref();
        let found = match route::route(url) {
            Engine::GalleryDl => self.ask_gallery_dl(url, file, tried).await,
            Engine::YtDlp => self.ask_yt_dlp(url, &[], file, tried, true).await,
            Engine::YtDlpThenGalleryDl => match self.ask_yt_dlp(url, &[], file, tried, true).await {
                Ok(found) => Ok(found),
                Err(_) => self.ask_gallery_dl(url, file, tried).await,
            },
        };
        match found {
            Ok((json, probe)) => Ok(Some(self.keep(url, json, probe).probe)),
            Err(SendError::Failed(message)) if message == probe::NO_READER => Ok(None),
            Err(error) => Err(error),
        }
    }

    /// Keeps a whole probe of `url` for its download.
    fn keep(&self, url: &str, json: String, probe: Probe) -> Probed {
        let probed = Probed { at: Instant::now(), json, probe, whole: true };
        lock(&self.probed).insert(url.to_string(), probed.clone());
        probed
    }

    /// `url`'s probe from the check still listing it (or one of its sections): a part of the list.
    fn listing_now(&self, url: &str) -> Option<Probed> {
        let check = lock(&self.check);
        let check = check.as_ref().filter(|check| check.probe.is_none())?;
        let (json, _, probe) = check.build()?;
        let (json, probe) = if check.url == url {
            (json, probe)
        } else {
            let (_, json) = probe::section_listings(&json).into_iter().find(|(link, _)| link == url)?;
            let probe = Info::parse(&json).ok()?.probe(url, check.thumbnail.clone());
            (json, probe)
        };
        Some(Probed { at: Instant::now(), json, probe, whole: false })
    }

    /// Changes the check `id`, if it's still the latest.
    fn in_check(&self, id: JobId, change: impl FnOnce(&mut Check)) {
        if let Some(check) = lock(&self.check).as_mut().filter(|check| check.id == id) {
            change(check);
        }
    }

    /// The check's probe of `url`: kept from the last hour when its listing was whole, else asked from the tool its
    /// route names, a yt-dlp list as it comes.
    async fn check_now(&self, url: &str, job: &Job) -> Result<Probed, SendError> {
        let kept = lock(&self.probed).get(url).cloned();
        if let Some(kept) = kept.filter(|probed| probed.whole && probed.at.elapsed() < PROBE_KEPT) {
            return Ok(kept);
        }
        let cookies = self.remembered(url);
        let tried = cookies.is_some();
        let file = cookies.as_deref().map(CookieFile::write).transpose()?;
        let file = file.as_ref();
        let (json, probe) = match route::route(url) {
            Engine::GalleryDl => self.ask_gallery_dl(url, file, tried).await?,
            Engine::YtDlp => self.list_yt_dlp(url, file, tried, job).await?,
            Engine::YtDlpThenGalleryDl => match self.list_yt_dlp(url, file, tried, job).await {
                Err(_) if job.cancelled() => return Err(SendError::Failed("Cancelled.".into())),
                Err(_) => self.ask_gallery_dl(url, file, tried).await?,
                found => found?,
            },
        };
        Ok(self.keep(url, json, probe))
    }

    /// yt-dlp's lazy listing of `url` into the check `job` runs, line by line (ADR-0030). Stopped, what was listed is
    /// the list. Each section is kept too, so it downloads without a new probe.
    async fn list_yt_dlp(
        &self,
        url: &str,
        file: Option<&CookieFile>,
        tried: bool,
        job: &Job,
    ) -> Result<(String, Probe), SendError> {
        let id = job.id();
        let mut fetching = None;
        let command = listing::command(&self.toolbox, url, file)?;
        let ran = engine::run_lines(command, "yt-dlp", job, |line| {
            let mut check = lock(&self.check);
            let Some(check) = check.as_mut().filter(|check| check.id == id) else { return };
            check.listing.add(line);
            job.items(check.listing.listed() as u64, None, None);
            // The list's thumbnail (its first video's for a Mix) is fetched once, while the listing goes on.
            if fetching.is_some() || check.listing.listed() == 0 {
                return;
            }
            let link = check.build().and_then(|(_, info, _)| info.thumbnail_url().map(str::to_string));
            let probes = self.clone();
            fetching = Some(tokio::spawn(async move {
                let picture = probe::thumbnail(link.as_deref()).await;
                probes.in_check(id, |check| check.thumbnail.clone_from(&picture));
                picture
            }));
        })
        .await;
        let (state, errors) = match ran {
            Err(_) if job.cancelled() => (State::Stopped, Vec::new()),
            Err(message) => return Err(classify(url, message, tried)),
            Ok((_, errors)) => (State::Done, errors),
        };
        let thumbnail = match fetching {
            Some(task) => task.await.ok().flatten(),
            None => None,
        };
        let mut built = None;
        self.in_check(id, |check| {
            (check.state, check.thumbnail) = (state, thumbnail);
            built = check.build();
        });
        let Some((json, info, mut probe)) = built else {
            return Err(classify(url, engine::error_text(&errors), tried));
        };
        // A single video's line is its only one, and comes last: its thumbnail is fetched now.
        if probe.count.is_none() {
            probe.thumbnail = probe::thumbnail(info.thumbnail_url()).await;
        }
        lock(&self.previews).extend(info.previews());
        for (link, json) in probe::section_listings(&json) {
            let section = Info::parse(&json)?.probe(&link, probe.thumbnail.clone());
            self.keep(&link, json, section);
        }
        Ok((json, probe))
    }

    /// gallery-dl's listing of `url`, as a probe with no JSON for yt-dlp.
    async fn ask_gallery_dl(&self, url: &str, file: Option<&CookieFile>, tried: bool) -> Result<(String, Probe), SendError> {
        let listing = gallery::list(&self.toolbox, url, file).await.map_err(|message| classify(url, message, tried))?;
        if listing.count == 0 {
            return Err(SendError::Failed("There's nothing to download here.".into()));
        }
        // A post or profile of videos only (X) has no image: its first video's first frame stands in.
        let thumbnail = match (&listing.preview, &listing.video) {
            (Some(image), _) => probe::thumbnail(Some(image)).await,
            (None, Some(video)) => thumbnail::video_frame(&self.toolbox, video).await,
            (None, None) => None,
        };
        Ok((String::new(), listing.probe(url, thumbnail)))
    }

    /// yt-dlp's JSON for `url` and the probe read from it. A channel's sections are kept as probes of their own.
    async fn ask_yt_dlp(
        &self,
        url: &str,
        headers: &[(String, String)],
        file: Option<&CookieFile>,
        tried: bool,
        known_only: bool,
    ) -> Result<(String, Probe), SendError> {
        let json = probe::fetch(&self.toolbox, url, headers, file, known_only)
            .await
            .map_err(|message| classify(url, message, tried))?;
        let info = Info::parse(&json)?;
        lock(&self.previews).extend(info.previews());
        let thumbnail = probe::thumbnail(info.thumbnail_url()).await;
        // A channel's sections come listed inside its listing: kept, so each downloads without a new probe.
        for (link, json) in probe::section_listings(&json) {
            let probe = Info::parse(&json)?.probe(&link, thumbnail.clone());
            self.keep(&link, json, probe);
        }
        let probe = info.probe(url, thumbnail);
        Ok((json, probe))
    }
}

#[cfg(test)]
mod tests {
    use super::{Probes, SendError, classify};
    use crate::jobs::{JobEvent, Jobs};
    use crate::media::Probe;
    use crate::tools::Toolbox;
    use crate::vault::{KeySource, Vault};
    use std::sync::Arc;
    use std::time::Duration;

    /// Probes with the tools in `SCRAPPY_TOOLS` (a folder with yt-dlp), for the tests that ask a site.
    fn live() -> (tempfile::TempDir, Probes) {
        let dir = tempfile::tempdir().unwrap();
        let tools = std::env::var("SCRAPPY_TOOLS").expect("SCRAPPY_TOOLS");
        let vault = Vault::open(&dir.path().join("vault"), KeySource::Fixed([1; 32]));
        let probes = Probes::new(Toolbox::new(tools.into()), vault);
        (dir, probes)
    }

    /// Checks `url` as the window does (ADR-0030) and returns its probe once the check ends.
    async fn check(probes: &Probes, url: &str) -> Result<Probe, String> {
        let (send, mut ended) = tokio::sync::mpsc::unbounded_channel();
        let jobs = Jobs::new(Arc::new(move |event| {
            if let JobEvent::Finished(end) = event {
                let _ = send.send(end);
            }
        }));
        let id = probes.list_link(&jobs, url)?;
        let end = ended.recv().await.unwrap();
        probes.listed_link(id).ok_or_else(|| format!("{end:?}"))
    }

    #[test]
    fn reads_not_found_as_a_sign_in_where_sites_hide_things() {
        let missing = || "Requested user could not be found".to_string();
        let instagram = classify("https://www.instagram.com/nasa/", missing(), false);
        assert_eq!(instagram, SendError::NeedsLogin { site: "instagram.com".into(), tried: false });
        assert_eq!(classify("https://bsky.app/profile/a", missing(), false), SendError::Failed(missing()));
    }

    /// Proves a channel's sections are counted one by one, list their videos, and are kept for their probes.
    /// Needs `SCRAPPY_TOOLS` (a folder with yt-dlp); run with `cargo test -p scrappy-core -- --ignored counts_a_channels`.
    #[tokio::test]
    #[ignore = "asks YouTube for a channel's listing"]
    async fn counts_a_channels_videos_and_shorts() {
        let (_dir, probes) = live();
        let probe = check(&probes, "https://www.youtube.com/@GDULA7").await.unwrap();
        let counts: Vec<_> = probe.sections.iter().map(|s| (s.title.as_str(), s.count, s.entries.len())).collect();
        println!("{} {:?} {counts:?}", probe.title, probe.count);
        assert!(probe.sections.len() >= 2, "{probe:?}");
        assert_eq!(probe.count, Some(probe.sections.iter().map(|s| s.count).sum()));
        assert!(probe.sections.iter().all(|s| s.entries.len() == s.count as usize), "{counts:?}");
        let videos = &probe.sections[0];
        assert_eq!(videos.title, "Videos");
        let kept = probes.look_up(&videos.url, &[], None).await.unwrap().probe;
        assert_eq!((kept.count, kept.entries.len()), (Some(videos.count), videos.entries.len()));
        let short = &probe.sections[1].entries[0];
        assert!(short.tall && !videos.entries[0].tall, "{short:?}");
        let picture = probes.preview(&short.url).await.unwrap();
        assert!(picture.starts_with("data:image/"));
        assert_eq!(probes.preview("https://example.com/not-listed").await, None);
    }

    /// Proves a Mix shows its first videos within seconds, and that Stop keeps what was listed as the list (ADR-0030).
    /// Needs `SCRAPPY_TOOLS`, like the test above.
    #[tokio::test]
    #[ignore = "asks YouTube for a Mix's listing"]
    async fn shows_a_mix_as_it_lists_and_stops() {
        let (_dir, probes) = live();
        let (send, mut ended) = tokio::sync::mpsc::unbounded_channel();
        let jobs = Jobs::new(Arc::new(move |event| {
            if let JobEvent::Finished(end) = event {
                let _ = send.send(end);
            }
        }));
        let started = std::time::Instant::now();
        let id = probes.list_link(&jobs, "https://www.youtube.com/watch?v=Tx8Ml-k4Z-8&list=RDdgKjUrX5YKs").unwrap();
        let first = loop {
            if let Some(probe) = probes.listed_link(id) {
                break probe;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        };
        println!("{} after {:?}: {} listed", first.title, started.elapsed(), first.entries.len());
        assert!(started.elapsed() < Duration::from_secs(10) && first.title.starts_with("Mix - "), "{first:?}");
        assert!(first.video.as_deref().is_some_and(|video| !video.contains("list=")), "{first:?}");
        jobs.cancel(id);
        ended.recv().await.unwrap();
        let stopped = probes.listed_link(id).unwrap();
        println!("stopped with {} after {:?}", stopped.entries.len(), started.elapsed());
        assert_eq!(stopped.count, Some(u32::try_from(stopped.entries.len()).unwrap()));
        assert!(!stopped.more && stopped.thumbnail.is_some(), "{stopped:?}");
    }

    /// Proves a video link that names a playlist probes as the playlist. Needs `SCRAPPY_TOOLS`, like the test above.
    #[tokio::test]
    #[ignore = "asks YouTube for a playlist's listing"]
    async fn reads_a_video_in_a_playlist_as_the_playlist() {
        let (_dir, probes) = live();
        let url = "https://www.youtube.com/watch?v=cvaIgq5j2Q8&list=PL0wqt_um4x0bsdViTJBmnl6KGMoSqxfZy";
        let probe = check(&probes, url).await.unwrap();
        println!("{} {:?} {}", probe.title, probe.count, probe.entries.len());
        assert!(probe.count.is_some_and(|count| count > 100) && !probe.entries.is_empty(), "{probe:?}");
    }

    /// Proves a video post on X probes through yt-dlp, with its qualities and a size. Needs `SCRAPPY_TOOLS` (a folder
    /// with yt-dlp); run with `cargo test -p scrappy-core -- --ignored reads_an_x_video`.
    #[tokio::test]
    #[ignore = "asks X for a public video post"]
    async fn reads_an_x_video_post_with_its_qualities() {
        let (_dir, probes) = live();
        let url = "https://twitter.com/i/web/status/910031516746514432";
        let probe = check(&probes, url).await.unwrap();
        println!("{} {:?}", probe.title, probe.heights);
        assert!(!probe.files && probe.count.is_none(), "{probe:?}");
        assert!(probe.heights.contains(&720) && probe.heights.contains(&360), "{probe:?}");
        assert!(probe.thumbnail.is_some());
        let size = probes.size(url, "editor-mp4", Some(360)).await.unwrap();
        assert!(size.is_some_and(|bytes| bytes > 1_000_000), "{size:?}");
    }
}

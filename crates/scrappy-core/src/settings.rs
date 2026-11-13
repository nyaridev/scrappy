//! Settings: the app's own settings (the library folder, the look, how downloads run, the player's volume, the Hunt
//! packs left out of its list, and the preferences: download extras and names, a proxy, each view's starting values,
//! how strict removing duplicates is, yt-dlp's updates) as JSON in the app config folder (ADR-0004, ADR-0034).

// ── Imports ───────────────────────────────
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::dedupe::Strictness;
use crate::hunt::ImageEngine;
use crate::net;

// ── Constants ─────────────────────────────
const FILE: &str = "settings.json";
const TEMP_FILE: &str = "settings.json.tmp";
/// Where an unreadable settings file goes, so starting over never destroys it.
const BROKEN_FILE: &str = "settings.broken.json";
/// The most downloads at once the setting allows (ADR-0017).
const MOST_AT_ONCE: u32 = 20;
/// The highest speed limit the setting takes, in MB/s: above any real connection.
const FASTEST: f64 = 10_000.0;
const MIB: f64 = 1024.0 * 1024.0;
/// A single download's place inside `Videos/` or `Audio/` until the user names it otherwise (ADR-0042).
const NAME_TEMPLATE: &str = "{site}/{channel}/{title} [{id}]";
/// The default before videos were filed by site; a file still holding it moves to the new one.
const OLD_NAME_TEMPLATE: &str = "{channel}/{title} [{id}]";
const MAX_TEMPLATE: usize = 200;

// ── Types ─────────────────────────────────
/// Everything Scrappy remembers between runs. Missing fields take their defaults, so older files still load.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct Settings {
    /// The library folder; none until first-run setup.
    pub library_root: Option<PathBuf>,
    pub look: Look,
    pub downloads: DownloadLimits,
    pub player: Player,
    /// Hunt packs left out of its Pack list, by id (ADR-0028).
    pub hidden_packs: Vec<String>,
    pub preferences: Preferences,
}

/// What the user chose beyond the basics (ADR-0034).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct Preferences {
    pub media: MediaOptions,
    /// An `http`, `https`, `socks5` or `socks5h` proxy for every request, without a user or password; empty for none.
    pub proxy: String,
    pub starts: StartValues,
    /// How alike pictures must be when Grab and Hunt remove copies on their own.
    pub duplicates: Strictness,
    /// Scrappy's own yt-dlp updates itself at every start.
    pub update_yt_dlp: bool,
    /// Scrappy starts, minimized, when the user signs in to the system (ADR-0036).
    pub start_with_system: bool,
    /// Scrappy looks for a newer version of itself at start and every 6 hours (ADR-0037).
    pub check_updates: bool,
    /// Closing the window hides it in the tray; Scrappy keeps running until it's quit from there (ADR-0038).
    pub close_to_tray: bool,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            media: MediaOptions::default(),
            proxy: String::new(),
            starts: StartValues::default(),
            duplicates: Strictness::Strict,
            update_yt_dlp: true,
            start_with_system: false,
            check_updates: true,
            close_to_tray: true,
        }
    }
}

/// What yt-dlp saves besides the video, where a single download goes, and what the Download view starts on.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct MediaOptions {
    /// Subtitles in `subtitle_languages`, embedded into the video.
    pub subtitles: bool,
    /// yt-dlp's list: `en`, `en,pl`, `en.*`.
    pub subtitle_languages: String,
    /// Auto-generated subtitles count too.
    pub auto_subtitles: bool,
    pub embed_metadata: bool,
    pub embed_thumbnail: bool,
    pub embed_chapters: bool,
    /// The preset the Download view starts on, by id; `None` for the first.
    pub preset: Option<String>,
    /// The highest quality it starts on, in pixels; `None` for the best.
    pub max_height: Option<u32>,
    /// A single download's path inside `Videos/` or `Audio/`, without its extension.
    pub name_template: String,
}

impl Default for MediaOptions {
    fn default() -> Self {
        Self {
            subtitles: false,
            subtitle_languages: "en".into(),
            auto_subtitles: false,
            embed_metadata: false,
            embed_thumbnail: false,
            embed_chapters: false,
            preset: None,
            max_height: None,
            name_template: NAME_TEMPLATE.into(),
        }
    }
}

/// What each view shows when it first opens.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct StartValues {
    pub grab: GrabStart,
    pub scrape: ScrapeStart,
    pub hunt: HuntStart,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct GrabStart {
    pub min_kb: u32,
    pub remove_duplicates: bool,
}

impl Default for GrabStart {
    fn default() -> Self {
        Self { min_kb: 10, remove_duplicates: true }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct ScrapeStart {
    pub depth: u32,
    pub pages: u32,
    pub delay_ms: u32,
    pub max_video_mb: u32,
}

impl Default for ScrapeStart {
    fn default() -> Self {
        Self { depth: 2, pages: 100, delay_ms: 500, max_video_mb: 200 }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct HuntStart {
    pub pages: u32,
    pub per_source: u32,
    pub min_kb: u32,
    pub engines: Vec<ImageEngine>,
}

impl Default for HuntStart {
    fn default() -> Self {
        let engines = vec![ImageEngine::DuckDuckGo, ImageEngine::Bing, ImageEngine::Yandex];
        Self { pages: 10, per_source: 50, min_kb: 20, engines }
    }
}

/// The Gallery player's sound, kept between runs (ADR-0023).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct Player {
    /// From 0 to 1.
    pub volume: f64,
    pub muted: bool,
}

impl Default for Player {
    fn default() -> Self {
        Self { volume: 1.0, muted: false }
    }
}

/// How downloads run (ADR-0017).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct DownloadLimits {
    /// How many downloads run at once, across the app; a playlist's videos count one each.
    pub at_once: u32,
    /// Each download's top speed in MB/s (MiB per second); 0 for none.
    pub speed_limit: f64,
}

impl Default for DownloadLimits {
    fn default() -> Self {
        Self { at_once: 10, speed_limit: 0.0 }
    }
}

/// How the window looks.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct Look {
    /// The system's own title bar instead of Scrappy's; applies at the next start (ADR-0011).
    pub native_frame: bool,
    /// The flowing shader behind the window; the glows stay when it's off.
    pub liquid_background: bool,
}

impl Default for Look {
    fn default() -> Self {
        Self { native_frame: false, liquid_background: true }
    }
}

/// The settings in memory and on disk. A change is kept only once it's saved. Clones share the same settings.
#[derive(Clone)]
pub struct Store {
    dir: PathBuf,
    current: Arc<Mutex<Settings>>,
}

// ── Private helpers ───────────────────────
fn read(dir: &Path) -> Settings {
    let path = dir.join(FILE);
    let Ok(text) = fs::read_to_string(&path) else {
        return Settings::default();
    };
    let mut settings: Settings = serde_json::from_str(&text).unwrap_or_else(|error| {
        tracing::warn!("Settings file unreadable ({error}); starting from defaults, old file kept as {BROKEN_FILE}");
        if let Err(error) = fs::rename(&path, dir.join(BROKEN_FILE)) {
            tracing::warn!("Couldn't keep the unreadable settings file: {error}");
        }
        Settings::default()
    });
    let name = &mut settings.preferences.media.name_template;
    if name == OLD_NAME_TEMPLATE {
        *name = NAME_TEMPLATE.into();
    }
    settings
}

/// Writes to a temporary file first, so a crash mid-write never leaves half a settings file.
fn write(dir: &Path, settings: &Settings) -> Result<(), String> {
    let text = serde_json::to_string_pretty(settings).map_err(|e| format!("Can't save the settings: {e}"))?;
    fs::create_dir_all(dir).map_err(|e| format!("Can't create the settings folder: {e}"))?;
    fs::write(dir.join(TEMP_FILE), text).map_err(|e| format!("Can't save the settings: {e}"))?;
    fs::rename(dir.join(TEMP_FILE), dir.join(FILE)).map_err(|e| format!("Can't save the settings: {e}"))
}

/// yt-dlp's language list, or `None` when it holds anything but letters, digits, `-`, `_`, `.`, `*` and commas.
fn languages(list: &str) -> Option<String> {
    let parts: Vec<&str> = list.split(',').map(str::trim).filter(|part| !part.is_empty()).collect();
    let fine = |part: &&str| part.chars().all(|c| c.is_ascii_alphanumeric() || "-_.*".contains(c));
    (!parts.is_empty() && parts.iter().all(fine)).then(|| parts.join(","))
}

// ── Public API ────────────────────────────
impl Preferences {
    /// The same preferences with every value in range, or what's wrong with the languages, the name or the proxy.
    pub fn checked(self) -> Result<Self, String> {
        let media = self.media;
        let subtitle_languages =
            languages(&media.subtitle_languages).ok_or("Subtitle languages are codes like en or en,pl.")?;
        let name_template = media.name_template.trim().to_string();
        if name_template.is_empty() || name_template.len() > MAX_TEMPLATE {
            return Err("The file name can't be empty or longer than 200 characters.".into());
        }
        let proxy = net::check_proxy(&self.proxy)?.unwrap_or_default();
        let (grab, scrape, hunt) = (self.starts.grab, self.starts.scrape, self.starts.hunt);
        let mut engines = hunt.engines;
        engines.dedup();
        Ok(Self {
            media: MediaOptions {
                subtitle_languages,
                name_template,
                max_height: media.max_height.filter(|height| (144..=8640).contains(height)),
                ..media
            },
            proxy,
            starts: StartValues {
                grab: GrabStart { min_kb: grab.min_kb.min(100_000), ..grab },
                scrape: ScrapeStart {
                    depth: scrape.depth.clamp(1, 10),
                    pages: scrape.pages.clamp(1, 10_000),
                    delay_ms: scrape.delay_ms.min(60_000),
                    max_video_mb: scrape.max_video_mb.clamp(1, 100_000),
                },
                hunt: HuntStart {
                    pages: hunt.pages.clamp(1, 50),
                    per_source: hunt.per_source.clamp(1, 1000),
                    min_kb: hunt.min_kb.min(100_000),
                    engines,
                },
            },
            ..self
        })
    }
}

impl DownloadLimits {
    /// The same limits, brought into range: 1 to 20 at once, a speed of 0 to 10,000 MB/s.
    pub fn checked(self) -> Self {
        let speed = if self.speed_limit.is_finite() { self.speed_limit.clamp(0.0, FASTEST) } else { 0.0 };
        Self { at_once: self.at_once.clamp(1, MOST_AT_ONCE), speed_limit: speed }
    }

    /// The speed limit in bytes per second, or `None` for no limit.
    pub fn bytes_per_second(&self) -> Option<u64> {
        let bytes = (self.speed_limit * MIB) as u64;
        (bytes > 0).then_some(bytes)
    }
}

impl Store {
    /// Loads the settings from `dir`, or the defaults when there are none yet.
    pub fn load(dir: &Path) -> Self {
        Self { dir: dir.to_path_buf(), current: Arc::new(Mutex::new(read(dir))) }
    }

    /// A copy of the current settings.
    pub fn get(&self) -> Settings {
        self.current.lock().unwrap_or_else(PoisonError::into_inner).clone()
    }

    /// The library folder, or an error before there is one.
    pub fn library_root(&self) -> Result<PathBuf, String> {
        self.get().library_root.ok_or_else(|| "Set up the library first.".into())
    }

    /// Saves a new look and returns the settings.
    pub fn set_look(&self, look: Look) -> Result<Settings, String> {
        self.update(|settings| settings.look = look)
    }

    /// Saves how downloads run, brought into range, and returns the settings.
    pub fn set_download_limits(&self, limits: DownloadLimits) -> Result<Settings, String> {
        self.update(|settings| settings.downloads = limits.checked())
    }

    /// Saves the player's sound, the volume brought into 0–1, and returns the settings.
    pub fn set_player(&self, player: Player) -> Result<Settings, String> {
        let volume = if player.volume.is_finite() { player.volume.clamp(0.0, 1.0) } else { 1.0 };
        self.update(|settings| settings.player = Player { volume, ..player })
    }

    /// Saves which Hunt packs are left out of its Pack list (each id once), and returns the settings.
    pub fn set_hidden_packs(&self, ids: Vec<String>) -> Result<Settings, String> {
        let mut seen = std::collections::HashSet::new();
        let ids = ids.into_iter().filter(|id| seen.insert(id.clone())).collect();
        self.update(|settings| settings.hidden_packs = ids)
    }

    /// Saves the preferences, brought into range, and returns the settings (ADR-0034).
    pub fn set_preferences(&self, preferences: Preferences) -> Result<Settings, String> {
        let preferences = preferences.checked()?;
        self.update(|settings| settings.preferences = preferences)
    }

    /// Saves the library folder and returns the settings.
    pub fn set_library_root(&self, root: PathBuf) -> Result<Settings, String> {
        self.update(|settings| settings.library_root = Some(root))
    }

    fn update(&self, change: impl FnOnce(&mut Settings)) -> Result<Settings, String> {
        let mut current = self.current.lock().unwrap_or_else(PoisonError::into_inner);
        let mut next = current.clone();
        change(&mut next);
        write(&self.dir, &next)?;
        *current = next.clone();
        Ok(next)
    }
}

#[cfg(test)]
mod tests {
    use super::{BROKEN_FILE, DownloadLimits, FILE, Look, MediaOptions, Player, Preferences, Settings, Store};
    use std::fs;

    #[test]
    fn starts_from_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let settings = Store::load(dir.path()).get();
        assert_eq!(settings, Settings::default());
        assert!(settings.look.liquid_background);
    }

    #[test]
    fn saves_and_loads_again() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::load(dir.path());
        store.set_library_root(dir.path().join("Library")).unwrap();
        store.set_look(Look { native_frame: true, liquid_background: false }).unwrap();
        assert_eq!(Store::load(dir.path()).get(), store.get());
    }

    #[test]
    fn fills_missing_fields_with_defaults() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(FILE), r#"{"look":{"nativeFrame":true}}"#).unwrap();
        let look = Store::load(dir.path()).get().look;
        assert!(look.native_frame && look.liquid_background);
    }

    #[test]
    fn moves_the_old_default_name_to_filing_by_site() {
        let dir = tempfile::tempdir().unwrap();
        let old_default = r#"{"preferences":{"media":{"nameTemplate":"{channel}/{title} [{id}]"}}}"#;
        fs::write(dir.path().join(FILE), old_default).unwrap();
        assert_eq!(Store::load(dir.path()).get().preferences.media.name_template, "{site}/{channel}/{title} [{id}]");
        fs::write(dir.path().join(FILE), r#"{"preferences":{"media":{"nameTemplate":"{title}"}}}"#).unwrap();
        assert_eq!(Store::load(dir.path()).get().preferences.media.name_template, "{title}");
    }

    #[test]
    fn keeps_download_limits_in_range() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::load(dir.path());
        assert_eq!(store.get().downloads, DownloadLimits { at_once: 10, speed_limit: 0.0 });
        assert_eq!(store.get().downloads.bytes_per_second(), None);
        let saved = store.set_download_limits(DownloadLimits { at_once: 99, speed_limit: f64::NAN }).unwrap();
        assert_eq!(saved.downloads, DownloadLimits { at_once: 20, speed_limit: 0.0 });
        let saved = store.set_download_limits(DownloadLimits { at_once: 0, speed_limit: 1.5 }).unwrap();
        assert_eq!((saved.downloads.at_once, saved.downloads.bytes_per_second()), (1, Some(1_572_864)));
    }

    #[test]
    fn keeps_the_players_sound_between_runs() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(Store::load(dir.path()).get().player, Player { volume: 1.0, muted: false });
        Store::load(dir.path()).set_player(Player { volume: 0.35, muted: true }).unwrap();
        assert_eq!(Store::load(dir.path()).get().player, Player { volume: 0.35, muted: true });
        let saved = Store::load(dir.path()).set_player(Player { volume: 7.0, muted: false }).unwrap();
        assert!((saved.player.volume - 1.0).abs() < f64::EPSILON);
        let saved = Store::load(dir.path()).set_player(Player { volume: f64::NAN, muted: false }).unwrap();
        assert!((saved.player.volume - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn keeps_the_hidden_packs_between_runs() {
        let dir = tempfile::tempdir().unwrap();
        assert!(Store::load(dir.path()).get().hidden_packs.is_empty());
        Store::load(dir.path()).set_hidden_packs(vec!["memes".into(), "space".into(), "memes".into()]).unwrap();
        assert_eq!(Store::load(dir.path()).get().hidden_packs, ["memes", "space"]);
    }

    #[test]
    fn keeps_an_unreadable_file_aside() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(FILE), "{ not json").unwrap();
        assert_eq!(Store::load(dir.path()).get(), Settings::default());
        assert_eq!(fs::read_to_string(dir.path().join(BROKEN_FILE)).unwrap(), "{ not json");
    }

    #[test]
    fn brings_preferences_into_range() {
        let languages = " en , pl ,".into();
        let media = MediaOptions { subtitle_languages: languages, max_height: Some(5), ..MediaOptions::default() };
        let mut wild = Preferences { media, ..Preferences::default() };
        wild.starts.scrape.depth = 99;
        wild.starts.hunt.per_source = 0;
        wild.proxy = " socks5h://127.0.0.1:9050/ ".into();
        let checked = wild.checked().unwrap();
        assert_eq!((checked.media.subtitle_languages.as_str(), checked.media.max_height), ("en,pl", None));
        assert_eq!((checked.starts.scrape.depth, checked.starts.hunt.per_source), (10, 1));
        assert_eq!(checked.proxy, "socks5h://127.0.0.1:9050");
        let bad = |change: fn(&mut Preferences)| {
            let mut preferences = Preferences::default();
            change(&mut preferences);
            preferences.checked().unwrap_err()
        };
        assert!(bad(|p| p.media.subtitle_languages = "en; rm -rf".into()).contains("languages"));
        assert!(bad(|p| p.media.name_template = "  ".into()).contains("file name"));
        assert!(bad(|p| p.proxy = "http://me:pw@host".into()).contains("password"));
    }

    #[test]
    fn reads_an_older_file_with_default_preferences() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(FILE), r#"{ "hiddenPacks": ["art"] }"#).unwrap();
        let settings = Store::load(dir.path()).get();
        assert_eq!(settings.preferences, Preferences::default());
        assert!(settings.preferences.update_yt_dlp && settings.preferences.media.name_template.contains("{title}"));
        assert!(settings.preferences.check_updates && !settings.preferences.start_with_system);
        assert!(settings.preferences.close_to_tray);
    }
}

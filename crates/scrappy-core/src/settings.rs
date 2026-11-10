//! Settings: the app's own settings (the library folder, the look) as JSON in the app config folder (ADR-0004).

// ── Imports ───────────────────────────────
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

// ── Constants ─────────────────────────────
const FILE: &str = "settings.json";
const TEMP_FILE: &str = "settings.json.tmp";
/// Where an unreadable settings file goes, so starting over never destroys it.
const BROKEN_FILE: &str = "settings.broken.json";

// ── Types ─────────────────────────────────
/// Everything Scrappy remembers between runs. Missing fields take their defaults, so older files still load.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct Settings {
    /// The library folder; none until first-run setup.
    pub library_root: Option<PathBuf>,
    pub look: Look,
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
    serde_json::from_str(&text).unwrap_or_else(|error| {
        tracing::warn!("Settings file unreadable ({error}); starting from defaults, old file kept as {BROKEN_FILE}");
        if let Err(error) = fs::rename(&path, dir.join(BROKEN_FILE)) {
            tracing::warn!("Couldn't keep the unreadable settings file: {error}");
        }
        Settings::default()
    })
}

/// Writes to a temporary file first, so a crash mid-write never leaves half a settings file.
fn write(dir: &Path, settings: &Settings) -> Result<(), String> {
    let text = serde_json::to_string_pretty(settings).map_err(|e| format!("Can't save the settings: {e}"))?;
    fs::create_dir_all(dir).map_err(|e| format!("Can't create the settings folder: {e}"))?;
    fs::write(dir.join(TEMP_FILE), text).map_err(|e| format!("Can't save the settings: {e}"))?;
    fs::rename(dir.join(TEMP_FILE), dir.join(FILE)).map_err(|e| format!("Can't save the settings: {e}"))
}

// ── Public API ────────────────────────────
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
    use super::{BROKEN_FILE, FILE, Look, Settings, Store};
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
    fn keeps_an_unreadable_file_aside() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(FILE), "{ not json").unwrap();
        assert_eq!(Store::load(dir.path()).get(), Settings::default());
        assert_eq!(fs::read_to_string(dir.path().join(BROKEN_FILE)).unwrap(), "{ not json");
    }
}

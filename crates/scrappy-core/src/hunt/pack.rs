//! Pack: source packs, TOML lists of the sites Hunt searches for a kind of topic, and whether it searches the whole
//! web too. The built-in ones come from `packs/`; the user's own are read from the config folder's `packs/`, and one
//! with a built-in pack's file name replaces it. Every site goes through the same search and finder (ADR-0013).

// ── Imports ───────────────────────────────
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

// ── Constants ─────────────────────────────
/// The built-in packs, in the order the window lists them.
const BUILT_IN: &[(&str, &str)] = &[
    ("general", include_str!("../../../../packs/general.toml")),
    ("games", include_str!("../../../../packs/games.toml")),
    ("art", include_str!("../../../../packs/art.toml")),
    ("people", include_str!("../../../../packs/people.toml")),
];
const FOLDER: &str = "packs";
const MAX_SITES: usize = 30;

// ── Types ─────────────────────────────────
/// A source pack.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Pack {
    /// Its file name without `.toml`.
    pub id: String,
    pub name: String,
    /// Also search the whole web, besides its sites.
    pub web: bool,
    /// Sites searched one by one (`site:fandom.com`).
    pub sites: Vec<String>,
}

/// A pack's file.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PackFile {
    name: String,
    #[serde(default = "yes")]
    web: bool,
    #[serde(default)]
    sites: Vec<String>,
}

/// The packs, and the user's packs that couldn't be read.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Packs {
    pub packs: Vec<Pack>,
    /// One line per unreadable pack: its file and why.
    pub problems: Vec<String>,
}

// ── Private helpers ───────────────────────
fn yes() -> bool {
    true
}

/// A site is a plain host name: `fandom.com`, `commons.wikimedia.org`.
pub(super) fn is_site(site: &str) -> bool {
    site.len() <= 100
        && site.contains('.')
        && !site.starts_with(['.', '-'])
        && site.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '.' || c == '-')
}

/// Reads one pack and checks its sites.
fn parse(id: &str, text: &str) -> Result<Pack, String> {
    let file: PackFile = toml::from_str(text).map_err(|e| format!("{id}.toml: {}", e.message().trim()))?;
    let sites = file.sites.iter().map(|site| site.trim().to_ascii_lowercase()).collect();
    let mut pack = Pack { id: id.to_string(), name: file.name, web: file.web, sites };
    if let Some(bad) = pack.sites.iter().find(|site| !is_site(site)) {
        return Err(format!("{id}.toml: \"{bad}\" isn't a site name like example.com."));
    }
    pack.sites.dedup();
    if pack.sites.len() > MAX_SITES {
        return Err(format!("{id}.toml: it lists more than {MAX_SITES} sites."));
    }
    if !pack.web && pack.sites.is_empty() {
        return Err(format!("{id}.toml: it has no sites and doesn't search the web."));
    }
    Ok(pack)
}

// ── Public API ────────────────────────────
/// The user's packs folder inside the app's config folder, made if it's missing.
pub fn folder(config: &Path) -> Result<PathBuf, String> {
    let dir = config.join(FOLDER);
    std::fs::create_dir_all(&dir).map_err(|e| format!("Can't make the packs folder: {e}"))?;
    Ok(dir)
}

/// Every pack: the built-in ones, then the user's from `dir`; a user pack with a built-in one's id replaces it.
pub fn packs(dir: Option<&Path>) -> Packs {
    let mut packs: Vec<Pack> =
        BUILT_IN.iter().map(|(id, text)| parse(id, text).unwrap_or_else(|error| panic!("built-in {error}"))).collect();
    let mut problems = Vec::new();
    let mut files: Vec<PathBuf> = dir
        .and_then(|dir| std::fs::read_dir(dir).ok())
        .map(|entries| entries.flatten().map(|entry| entry.path()).collect())
        .unwrap_or_default();
    files.retain(|file| file.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("toml")));
    files.sort();
    for file in files {
        let id = file.file_stem().unwrap_or_default().to_string_lossy().into_owned();
        let read = std::fs::read_to_string(&file).map_err(|e| format!("{id}.toml: {e}"));
        match read.and_then(|text| parse(&id, &text)) {
            Ok(pack) => match packs.iter_mut().find(|known| known.id == pack.id) {
                Some(known) => *known = pack,
                None => packs.push(pack),
            },
            Err(error) => problems.push(error),
        }
    }
    Packs { packs, problems }
}

#[cfg(test)]
mod tests {
    use super::{is_site, packs, parse};

    #[test]
    fn reads_the_built_in_packs() {
        let all = packs(None);
        assert!(all.problems.is_empty());
        let ids: Vec<&str> = all.packs.iter().map(|pack| pack.id.as_str()).collect();
        assert_eq!(ids, ["general", "games", "art", "people"]);
        assert!(all.packs[1].sites.contains(&"fandom.com".to_string()));
    }

    #[test]
    fn reads_the_users_packs() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("games.toml"), "name = \"Mine\"\nweb = false\nsites = [\"Metin2.Fandom.com\"]\n").unwrap();
        std::fs::write(dir.path().join("broken.toml"), "name = 3").unwrap();
        std::fs::write(dir.path().join("notes.txt"), "not a pack").unwrap();
        let all = packs(Some(dir.path()));
        assert_eq!((all.packs[1].name.as_str(), all.packs[1].sites[0].as_str()), ("Mine", "metin2.fandom.com"));
        assert_eq!(all.problems.len(), 1);
        assert!(all.problems[0].starts_with("broken.toml"));
        assert!(parse("x", "name = \"X\"\nsites = [\"https://a.com/\"]\n").is_err());
        assert!(parse("x", "name = \"X\"\nweb = false\n").is_err());
        assert!(parse("x", "name = \"X\"\nkind = \"json\"\n").is_err());
        assert!(is_site("commons.wikimedia.org") && !is_site("a b.com") && !is_site("localhost") && !is_site("-a.com"));
    }
}

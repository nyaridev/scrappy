//! Pack: source packs, TOML lists of the sites Hunt searches for a kind of topic, and whether it searches the whole
//! web too. The built-in ones come from `packs/`; the user's own are read from the config folder's `packs/`, and one
//! with a built-in pack's file name replaces it. Every site goes through the same search and finder (ADR-0013). A pack
//! edited in the window is saved there as the user's (ADR-0020); the user's own packs are created and removed there,
//! and built-in ones keep their names (ADR-0028).

// ── Imports ───────────────────────────────
use std::collections::HashSet;
use std::path::{Path, PathBuf};

use reqwest::Url;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

// ── Constants ─────────────────────────────
/// The built-in packs, in the order the window lists them: each group's together.
const BUILT_IN: &[(&str, &str)] = &[
    ("general", include_str!("../../../../packs/general.toml")),
    ("wallpapers", include_str!("../../../../packs/wallpapers.toml")),
    ("stock", include_str!("../../../../packs/stock.toml")),
    ("people", include_str!("../../../../packs/people.toml")),
    ("movies", include_str!("../../../../packs/movies.toml")),
    ("games", include_str!("../../../../packs/games.toml")),
    ("anime", include_str!("../../../../packs/anime.toml")),
    ("memes", include_str!("../../../../packs/memes.toml")),
    ("art", include_str!("../../../../packs/art.toml")),
    ("museums", include_str!("../../../../packs/museums.toml")),
    ("architecture", include_str!("../../../../packs/architecture.toml")),
    ("nature", include_str!("../../../../packs/nature.toml")),
    ("space", include_str!("../../../../packs/space.toml")),
    ("history", include_str!("../../../../packs/history.toml")),
];
const FOLDER: &str = "packs";
const MAX_SITES: usize = 30;
const MAX_NAME: usize = 60;
/// The longest id made from a new pack's name.
const MAX_ID: usize = 40;

// ── Types ─────────────────────────────────
/// A source pack.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Pack {
    /// Its file name without `.toml`.
    pub id: String,
    pub name: String,
    /// The heading Hunt lists it under; `None` for a user's own pack that names none.
    #[serde(default)]
    pub group: Option<String>,
    /// Also search any site (the web search), besides its sites.
    pub web: bool,
    /// Sites searched one by one (`site:fandom.com`).
    pub sites: Vec<String>,
    /// Sites in `sites` switched off: Hunt shows them unticked.
    #[serde(default)]
    pub off: Vec<String>,
    /// One of Scrappy's own packs: it can't be removed or renamed (ADR-0028). Set by core; ignored when sent to it.
    #[serde(default)]
    pub built_in: bool,
}

/// A pack's file.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PackFile {
    name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    group: Option<String>,
    #[serde(default = "yes")]
    web: bool,
    #[serde(default)]
    sites: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    off: Vec<String>,
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

/// A pack's id can be a file name on every system: letters, digits, `-` and `_`.
fn is_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= MAX_NAME && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// The site name in what the user typed: `https://www.ArtStation.com/x` → `artstation.com`.
fn site_name(input: &str) -> String {
    let text = input.trim().to_ascii_lowercase();
    let host = match Url::parse(&text) {
        Ok(url) if matches!(url.scheme(), "http" | "https") => url.host_str().unwrap_or_default().to_string(),
        _ => text.split('/').next().unwrap_or_default().to_string(),
    };
    host.strip_prefix("www.").map(str::to_string).unwrap_or(host)
}

/// A pack from its parts, checked: a name, plain host names (each once), at most `MAX_SITES`, and somewhere to search.
/// `off` keeps only sites the pack lists.
fn checked(id: &str, file: PackFile) -> Result<Pack, String> {
    let name = file.name.trim().to_string();
    if name.is_empty() || name.chars().count() > MAX_NAME {
        return Err(format!("The name must be 1 to {MAX_NAME} characters."));
    }
    let group = file.group.map(|group| group.trim().to_string()).filter(|group| !group.is_empty());
    if group.as_ref().is_some_and(|group| group.chars().count() > MAX_NAME) {
        return Err(format!("The group must be at most {MAX_NAME} characters."));
    }
    let mut seen = HashSet::new();
    let sites: Vec<String> = file
        .sites
        .iter()
        .map(|site| site.trim().to_ascii_lowercase())
        .filter(|site| seen.insert(site.clone()))
        .collect();
    if let Some(bad) = sites.iter().find(|site| !is_site(site)) {
        return Err(format!("\"{bad}\" isn't a site name like example.com."));
    }
    if sites.len() > MAX_SITES {
        return Err(format!("A pack lists at most {MAX_SITES} sites."));
    }
    if !file.web && sites.is_empty() {
        return Err("It has no sites and doesn't search the web.".into());
    }
    let off = file.off.iter().map(|site| site.trim().to_ascii_lowercase()).filter(|site| sites.contains(site)).collect();
    Ok(Pack { id: id.to_string(), name, group, web: file.web, sites, off, built_in: false })
}

/// Whether `id` is a built-in pack's.
fn is_built_in(id: &str) -> bool {
    BUILT_IN.iter().any(|(built_in, _)| *built_in == id)
}

/// An id for a new pack made from its name: lowercase letters and digits joined by `-` (`My cats!` → `my-cats`).
fn id_from(name: &str) -> String {
    let words: String = name.chars().map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { ' ' }).collect();
    let id: String = words.split_whitespace().collect::<Vec<_>>().join("-").chars().take(MAX_ID).collect();
    let id = id.trim_end_matches('-');
    if id.is_empty() { "pack".into() } else { id.to_string() }
}

/// Checks `pack` as a file is, with each site the user typed (or pasted as a link) kept as its site name, and writes
/// it as `<id>.toml` in `dir`.
fn write(dir: &Path, pack: Pack) -> Result<(), String> {
    let names = |sites: &[String]| sites.iter().map(|site| site_name(site)).collect();
    let file = PackFile { name: pack.name, group: pack.group, web: pack.web, sites: names(&pack.sites), off: names(&pack.off) };
    let pack = checked(&pack.id, file)?;
    let file = PackFile { name: pack.name, group: pack.group, web: pack.web, sites: pack.sites, off: pack.off };
    let text = toml::to_string(&file).map_err(|e| format!("Can't write the pack: {e}"))?;
    let path = dir.join(format!("{}.toml", pack.id));
    // Written beside it, then renamed: a pack is never left half-written.
    let part = path.with_extension("toml.part");
    std::fs::write(&part, text).and_then(|()| std::fs::rename(&part, &path)).map_err(|e| format!("Can't save the pack: {e}"))
}

/// Reads one pack and checks its sites.
fn parse(id: &str, text: &str) -> Result<Pack, String> {
    let file: PackFile = toml::from_str(text).map_err(|e| format!("{id}.toml: {}", e.message().trim()))?;
    checked(id, file).map_err(|error| format!("{id}.toml: {error}"))
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
    let mut packs: Vec<Pack> = BUILT_IN
        .iter()
        .map(|(id, text)| Pack { built_in: true, ..parse(id, text).unwrap_or_else(|error| panic!("built-in {error}")) })
        .collect();
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
                // A user's copy of a built-in pack keeps its name, and its heading unless it names its own.
                Some(known) => {
                    let name = std::mem::take(&mut known.name);
                    *known = Pack { name, group: pack.group.clone().or(known.group.take()), built_in: true, ..pack };
                }
                None => packs.push(pack),
            },
            Err(error) => problems.push(error),
        }
    }
    Packs { packs, problems }
}

/// Saves `pack` as the user's, `<id>.toml` in `dir`, checked as a file is, with each site the user typed (or pasted as
/// a link) kept as its site name. Only a pack that's there can be saved; a built-in one keeps its name. Returns every
/// pack, read again.
pub fn save(dir: &Path, pack: Pack) -> Result<Packs, String> {
    let known = packs(Some(dir)).packs.into_iter().find(|known| known.id == pack.id);
    let Some(known) = known.filter(|_| is_id(&pack.id)) else {
        return Err("That pack isn't there anymore.".into());
    };
    let name = if known.built_in { known.name } else { pack.name };
    write(dir, Pack { name, ..pack })?;
    Ok(packs(Some(dir)))
}

/// Saves `pack` as a new pack of the user's, under an id made from its name that no pack (nor any file in `dir`) has,
/// so it never replaces a built-in one (ADR-0028). Returns the new pack's id.
pub fn create(dir: &Path, pack: Pack) -> Result<String, String> {
    let taken: HashSet<String> = packs(Some(dir)).packs.into_iter().map(|known| known.id).collect();
    let base = id_from(&pack.name);
    let free = |id: &String| !taken.contains(id) && !dir.join(format!("{id}.toml")).exists();
    let id = (1..).map(|n| if n == 1 { base.clone() } else { format!("{base}-{n}") }).find(free).unwrap_or(base);
    write(dir, Pack { id: id.clone(), built_in: false, ..pack })?;
    Ok(id)
}

/// Deletes the user's own pack `id` from `dir`; built-in packs can't be removed (ADR-0028). Returns every pack, read
/// again.
pub fn remove(dir: &Path, id: &str) -> Result<Packs, String> {
    if is_built_in(id) {
        return Err("Built-in packs can't be removed.".into());
    }
    let path = dir.join(format!("{id}.toml"));
    if !is_id(id) || !path.is_file() {
        return Err("That pack isn't there anymore.".into());
    }
    std::fs::remove_file(&path).map_err(|e| format!("Can't remove the pack: {e}"))?;
    Ok(packs(Some(dir)))
}

#[cfg(test)]
mod tests {
    use super::{Pack, create, id_from, is_site, packs, parse, remove, save, site_name};

    #[test]
    fn reads_the_built_in_packs() {
        let all = packs(None);
        assert!(all.problems.is_empty());
        let ids: Vec<&str> = all.packs.iter().map(|pack| pack.id.as_str()).collect();
        assert_eq!((ids.len(), &ids[..3]), (14, &["general", "wallpapers", "stock"][..]));
        assert!(all.packs.iter().find(|pack| pack.id == "games").unwrap().sites.contains(&"fandom.com".to_string()));
        // The built-in packs name every place they search (ADR-0024).
        assert!(all.packs.iter().all(|pack| !pack.web && !pack.sites.is_empty()));
        // Each has a group, and each group's packs come together, as the window lists them.
        let mut groups: Vec<&str> = Vec::new();
        for group in all.packs.iter().map(|pack| pack.group.as_deref().expect("a built-in pack's group")) {
            if groups.last() != Some(&group) {
                assert!(!groups.contains(&group), "{group} is split");
                groups.push(group);
            }
        }
        assert_eq!(groups.len(), 4);
    }

    #[test]
    fn reads_the_users_packs() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("games.toml"), "name = \"Mine\"\nweb = false\nsites = [\"Metin2.Fandom.com\"]\n").unwrap();
        std::fs::write(dir.path().join("broken.toml"), "name = 3").unwrap();
        std::fs::write(dir.path().join("notes.txt"), "not a pack").unwrap();
        let all = packs(Some(dir.path()));
        let games = all.packs.iter().find(|pack| pack.id == "games").unwrap();
        // A copy of a built-in pack keeps the built-in name (ADR-0028).
        assert_eq!((games.name.as_str(), games.sites[0].as_str(), games.built_in), ("Games", "metin2.fandom.com", true));
        // A copy of a built-in pack that names no group stays under the built-in one's.
        assert_eq!(games.group.as_deref(), Some("Entertainment"));
        assert_eq!(parse("x", "name = \"X\"
group = \" Mine \"
sites = [\"a.com\"]
").unwrap().group.as_deref(), Some("Mine"));
        assert_eq!(parse("x", "name = \"X\"
sites = [\"a.com\"]
").unwrap().group, None);
        assert_eq!(all.problems.len(), 1);
        assert!(all.problems[0].starts_with("broken.toml"));
        assert!(parse("x", "name = \"X\"\nsites = [\"https://a.com/\"]\n").is_err());
        assert!(parse("x", "name = \"X\"\nweb = false\n").is_err());
        assert!(parse("x", "name = \"X\"\nkind = \"json\"\n").is_err());
        assert!(is_site("commons.wikimedia.org") && !is_site("a b.com") && !is_site("localhost") && !is_site("-a.com"));
    }

    #[test]
    fn saves_an_edited_pack_as_the_users() {
        let dir = tempfile::tempdir().unwrap();
        let mut games = packs(None).packs.into_iter().find(|pack| pack.id == "games").unwrap();
        games.name = " My games ".into();
        games.sites = vec!["fandom.com".into(), "https://www.ArtStation.com/artwork/1".into(), "fandom.com".into()];
        games.off = vec!["artstation.com".into(), "nowhere.com".into()];
        let all = save(dir.path(), games.clone()).unwrap();
        let saved = all.packs.iter().find(|pack| pack.id == "games").unwrap();
        assert_eq!(saved.name, "Games");
        assert_eq!(saved.sites, ["fandom.com", "artstation.com"]);
        assert_eq!(saved.off, ["artstation.com"]);
        assert!(dir.path().join("games.toml").is_file() && !dir.path().join("games.toml.part").exists());

        let refuse = |pack: Pack| save(dir.path(), pack).unwrap_err();
        assert!(refuse(Pack { id: "../evil".into(), ..games.clone() }).contains("isn't there"));
        assert!(refuse(Pack { id: "con".into(), ..games.clone() }).contains("isn't there"));
        assert!(refuse(Pack { sites: vec!["a b".into()], ..games.clone() }).contains("site name"));
        assert!(refuse(Pack { web: false, sites: Vec::new(), ..games.clone() }).contains("no sites"));
        assert_eq!(site_name(" Fandom.com/wiki/Cat "), "fandom.com");
    }

    #[test]
    fn creates_and_removes_the_users_packs() {
        let dir = tempfile::tempdir().unwrap();
        let new = |name: &str| Pack {
            id: "ignored".into(),
            name: name.into(),
            group: None,
            web: false,
            sites: vec!["https://www.Example.com/cats".into()],
            off: Vec::new(),
            built_in: true,
        };
        assert_eq!(create(dir.path(), new(" My cats! ")).unwrap(), "my-cats");
        assert_eq!(create(dir.path(), new("My cats")).unwrap(), "my-cats-2");
        // A new pack named like a built-in one gets its own id, so it never replaces it.
        assert_eq!(create(dir.path(), new("General")).unwrap(), "general-2");
        assert_eq!(create(dir.path(), new("ねこ")).unwrap(), "pack");
        assert_eq!(id_from("  --A  b--c "), "a-b-c");
        assert!(create(dir.path(), Pack { sites: Vec::new(), ..new("Empty") }).unwrap_err().contains("no sites"));
        assert!(create(dir.path(), new(" ")).unwrap_err().contains("name"));

        let all = packs(Some(dir.path()));
        let mine = all.packs.iter().find(|pack| pack.id == "my-cats").unwrap();
        assert_eq!((mine.name.as_str(), mine.sites[0].as_str(), mine.built_in, mine.group.as_deref()), ("My cats!", "example.com", false, None));

        // Renaming the user's own pack works; a built-in one keeps its name.
        let renamed = save(dir.path(), Pack { name: "Kittens".into(), ..mine.clone() }).unwrap();
        assert!(renamed.packs.iter().any(|pack| pack.id == "my-cats" && pack.name == "Kittens"));
        let general = renamed.packs.iter().find(|pack| pack.id == "general").unwrap().clone();
        let kept = save(dir.path(), Pack { name: "Mine".into(), ..general }).unwrap();
        assert!(kept.packs.iter().any(|pack| pack.id == "general" && pack.name == "General" && pack.built_in));

        let left = remove(dir.path(), "my-cats").unwrap();
        assert!(!left.packs.iter().any(|pack| pack.id == "my-cats") && !dir.path().join("my-cats.toml").exists());
        assert!(remove(dir.path(), "general").unwrap_err().contains("can't be removed"));
        assert!(remove(dir.path(), "my-cats").unwrap_err().contains("isn't there"));
        assert!(remove(dir.path(), "../settings").unwrap_err().contains("isn't there"));
    }
}

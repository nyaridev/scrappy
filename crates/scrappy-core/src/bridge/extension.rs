//! Extension: the browser extension's files for this version of the app, fetched from its GitHub release, checked
//! against GitHub's SHA-256 for them, and unpacked into the data folder for the user to load unpacked (ADR-0040).

// ── Imports ───────────────────────────────
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::net;

// ── Constants ─────────────────────────────
const RELEASES: &str = "https://api.github.com/repos/nyaridev/scrappy/releases/tags";
/// The folder in the data folder that holds the unpacked extension; the browser loads it from here.
const FOLDER: &str = "browser-extension";
/// The extension's zip is a few hundred kB; anything far bigger isn't it.
const MAX_BYTES: usize = 20 * 1024 * 1024;

// ── Types ─────────────────────────────────
#[derive(Deserialize)]
struct Release {
    assets: Vec<Asset>,
}

#[derive(Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
    /// `sha256:<hex>`.
    digest: Option<String>,
}

#[derive(Deserialize)]
struct Manifest {
    version: String,
}

// ── Private helpers ───────────────────────
/// The extension's version in `folder`, if an unpacked extension is there.
fn version_in(folder: &Path) -> Option<String> {
    let text = fs::read_to_string(folder.join("manifest.json")).ok()?;
    serde_json::from_str::<Manifest>(&text).ok().map(|manifest| manifest.version)
}

/// The asset for `version`'s Chromium extension in the release's list.
fn pick(assets: Vec<Asset>, version: &str) -> Result<Asset, String> {
    let name = format!("scrappy-extension-{version}-chrome.zip");
    assets.into_iter().find(|asset| asset.name == name).ok_or_else(|| format!("The release has no {name}."))
}

/// Checks `bytes` against GitHub's digest for them.
fn check(bytes: &[u8], digest: Option<&str>) -> Result<(), String> {
    let expected = digest.and_then(|digest| digest.strip_prefix("sha256:")).ok_or("GitHub gave no checksum for it.")?;
    let found = format!("{:x}", Sha256::digest(bytes));
    if found == expected.to_ascii_lowercase() { Ok(()) } else { Err("The download doesn't match its checksum.".into()) }
}

/// Unpacks `zip` into `data/<FOLDER>`, replacing what's there only once the new files are all out.
fn unpack(zip: &[u8], data: &Path) -> Result<PathBuf, String> {
    let folder = data.join(FOLDER);
    let fresh = data.join(format!("{FOLDER}.new"));
    let failed = |e: &dyn std::fmt::Display| format!("Can't unpack the extension: {e}");
    let _ = fs::remove_dir_all(&fresh);
    // `extract` keeps every entry inside the folder it's given.
    zip::ZipArchive::new(Cursor::new(zip)).and_then(|mut archive| archive.extract(&fresh)).map_err(|e| failed(&e))?;
    if version_in(&fresh).is_none() {
        let _ = fs::remove_dir_all(&fresh);
        return Err("The download isn't the extension.".into());
    }
    if folder.exists() {
        fs::remove_dir_all(&folder).map_err(|e| failed(&e))?;
    }
    fs::rename(&fresh, &folder).map_err(|e| failed(&e))?;
    Ok(folder)
}

// ── Public API ────────────────────────────
/// The unpacked extension for `version`, if it's in the data folder.
pub fn extension_files(data: &Path, version: &str) -> Option<PathBuf> {
    let folder = data.join(FOLDER);
    (version_in(&folder).as_deref() == Some(version)).then_some(folder)
}

/// The extension for `version`: the copy in the data folder, else fetched from that version's release, checked and
/// unpacked there. Returns its folder.
pub async fn fetch_extension(data: &Path, version: &str) -> Result<PathBuf, String> {
    if let Some(folder) = extension_files(data, version) {
        return Ok(folder);
    }
    let failed = |e: reqwest::Error| format!("Can't download the extension: {e}");
    let client = net::client().build().map_err(failed)?;
    let response = client.get(format!("{RELEASES}/v{version}")).send().await.map_err(failed)?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Err(format!("Scrappy {version} has no release on GitHub to get the extension from."));
    }
    let text = response.error_for_status().map_err(failed)?.text().await.map_err(failed)?;
    let release: Release = serde_json::from_str(&text).map_err(|e| format!("Can't read the release: {e}"))?;
    let asset = pick(release.assets, version)?;
    let bytes = client.get(&asset.browser_download_url).send().await.and_then(|r| r.error_for_status()).map_err(failed)?;
    let bytes = bytes.bytes().await.map_err(failed)?;
    if bytes.len() > MAX_BYTES {
        return Err("The download is too large to be the extension.".into());
    }
    check(&bytes, asset.digest.as_deref())?;
    let folder = unpack(&bytes, data)?;
    tracing::info!("Fetched the browser extension");
    Ok(folder)
}

#[cfg(test)]
mod tests {
    use super::{Asset, check, extension_files, pick, unpack};
    use sha2::{Digest, Sha256};
    use std::io::{Cursor, Write};

    fn zip_of(files: &[(&str, &str)]) -> Vec<u8> {
        let mut out = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for (name, text) in files {
            out.start_file(*name, zip::write::SimpleFileOptions::default()).unwrap();
            out.write_all(text.as_bytes()).unwrap();
        }
        out.finish().unwrap().into_inner()
    }

    #[test]
    fn checks_the_download_against_githubs_digest() {
        let bytes = b"extension";
        let digest = format!("sha256:{:x}", Sha256::digest(bytes));
        assert!(check(bytes, Some(&digest)).is_ok());
        assert!(check(b"something else", Some(&digest)).is_err());
        assert!(check(bytes, None).is_err());
    }

    #[test]
    fn picks_the_chromium_zip_for_the_version() {
        let asset = |name: &str| Asset { name: name.into(), browser_download_url: String::new(), digest: None };
        let assets = vec![asset("scrappy-extension-0.2.0-firefox.zip"), asset("scrappy-extension-0.2.0-chrome.zip")];
        assert_eq!(pick(assets, "0.2.0").unwrap().name, "scrappy-extension-0.2.0-chrome.zip");
        assert!(pick(vec![asset("Scrappy_0.2.0_x64-setup.exe")], "0.2.0").is_err());
    }

    #[test]
    fn unpacks_and_replaces_an_older_copy() {
        let data = tempfile::tempdir().unwrap();
        let old = zip_of(&[("manifest.json", r#"{"version":"0.1.0"}"#), ("old.js", "")]);
        unpack(&old, data.path()).unwrap();
        assert!(extension_files(data.path(), "0.1.0").is_some() && extension_files(data.path(), "0.2.0").is_none());
        let new = zip_of(&[("manifest.json", r#"{"version":"0.2.0"}"#), ("content-scripts/buttons.js", "")]);
        let folder = unpack(&new, data.path()).unwrap();
        assert_eq!(extension_files(data.path(), "0.2.0"), Some(folder.clone()));
        assert!(folder.join("content-scripts/buttons.js").is_file() && !folder.join("old.js").exists());
        assert!(unpack(&zip_of(&[("readme.txt", "")]), data.path()).is_err());
        assert!(extension_files(data.path(), "0.2.0").is_some(), "a bad download leaves the copy alone");
    }
}

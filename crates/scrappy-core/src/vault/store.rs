//! Store: remembered sign-ins, one entry per site, in `<app data>/vault.bin`, encrypted with ChaCha20-Poly1305
//! under a key kept in the OS keychain (ADR-0005). Nothing touches the keychain until a sign-in is remembered.

// ── Imports ───────────────────────────────
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{SystemTime, UNIX_EPOCH};

use chacha20poly1305::aead::{Aead, AeadCore, KeyInit, OsRng};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::cookie::{Cookie, remove_leftover_files};

// ── Constants ─────────────────────────────
const FILE: &str = "vault.bin";
const TEMP_FILE: &str = "vault.bin.tmp";
/// The file's first bytes: what it is and its format's version.
const MAGIC: &[u8; 4] = b"SCV1";
const NONCE_LEN: usize = 12;
const KEYCHAIN_SERVICE: &str = "Scrappy";
const KEYCHAIN_ACCOUNT: &str = "vault-key";

// ── Types ─────────────────────────────────
/// Where the encryption key comes from.
pub enum KeySource {
    /// The OS keychain (Windows Credential Manager, macOS Keychain, the Secret Service on Linux).
    Keychain,
    /// A fixed key, for tests.
    Fixed([u8; 32]),
}

/// A remembered site, for Settings.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RememberedSite {
    pub site: String,
    pub cookies: u32,
    /// Unix seconds.
    #[ts(type = "number")]
    pub saved_at: i64,
}

#[derive(Default, Serialize, Deserialize)]
struct Contents {
    sites: BTreeMap<String, Saved>,
}

#[derive(Serialize, Deserialize)]
struct Saved {
    saved_at: i64,
    cookies: Vec<Cookie>,
}

/// The remembered sign-ins. Clones share the same file and key.
#[derive(Clone)]
pub struct Vault {
    inner: Arc<Inner>,
}

struct Inner {
    file: PathBuf,
    source: KeySource,
    /// The key once read, and one lock for every read and write of the file.
    key: Mutex<Option<Key>>,
}

// ── Private helpers ───────────────────────
fn now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |since| since.as_secs() as i64)
}

fn keychain_entry() -> Result<keyring::Entry, String> {
    keyring::Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT).map_err(|e| format!("Can't reach the system keychain: {e}"))
}

/// The key from the keychain; `create` makes and stores one when there's none.
fn key_from(source: &KeySource, create: bool) -> Result<Option<Key>, String> {
    match source {
        KeySource::Fixed(bytes) => Ok(Some(*Key::from_slice(bytes))),
        KeySource::Keychain => {
            let entry = keychain_entry()?;
            match entry.get_secret() {
                Ok(bytes) if bytes.len() == 32 => Ok(Some(*Key::from_slice(&bytes))),
                Ok(_) | Err(keyring::Error::NoEntry) if create => {
                    let key = ChaCha20Poly1305::generate_key(&mut OsRng);
                    entry.set_secret(&key).map_err(|e| format!("Can't save the key in the system keychain: {e}"))?;
                    Ok(Some(key))
                }
                Ok(_) | Err(keyring::Error::NoEntry) => Ok(None),
                Err(e) => Err(format!("Can't read the system keychain: {e}")),
            }
        }
    }
}

fn decrypt(key: &Key, bytes: &[u8]) -> Result<Contents, String> {
    let unreadable = || "Remembered sign-ins can't be read.".to_string();
    let rest = bytes.strip_prefix(MAGIC.as_slice()).ok_or_else(unreadable)?;
    if rest.len() < NONCE_LEN {
        return Err(unreadable());
    }
    let (nonce, sealed) = rest.split_at(NONCE_LEN);
    let plain = ChaCha20Poly1305::new(key).decrypt(Nonce::from_slice(nonce), sealed).map_err(|_| unreadable())?;
    serde_json::from_slice(&plain).map_err(|_| unreadable())
}

fn encrypt(key: &Key, contents: &Contents) -> Result<Vec<u8>, String> {
    let plain = serde_json::to_vec(contents).map_err(|e| format!("Can't save the sign-ins: {e}"))?;
    let nonce = ChaCha20Poly1305::generate_nonce(&mut OsRng);
    let sealed =
        ChaCha20Poly1305::new(key).encrypt(&nonce, plain.as_slice()).map_err(|_| "Can't save the sign-ins.".to_string())?;
    Ok([MAGIC.as_slice(), nonce.as_slice(), sealed.as_slice()].concat())
}

impl Inner {
    /// Reads the file; an empty vault when there's no file yet, or when its key is gone from the keychain (the
    /// contents are lost then, and the next save starts over with a new key).
    fn read(&self, key: &mut Option<Key>) -> Result<Contents, String> {
        let Ok(bytes) = fs::read(&self.file) else { return Ok(Contents::default()) };
        if key.is_none() {
            *key = key_from(&self.source, false)?;
        }
        match key {
            Some(key) => decrypt(key, &bytes),
            None => {
                tracing::warn!("The vault's key is gone from the keychain; starting a new vault");
                Ok(Contents::default())
            }
        }
    }

    fn write(&self, key: &mut Option<Key>, contents: &Contents) -> Result<(), String> {
        if key.is_none() {
            *key = key_from(&self.source, true)?;
        }
        let key = key.as_ref().ok_or("Can't save the key in the system keychain.")?;
        let error = |e: std::io::Error| format!("Can't save the sign-ins: {e}");
        if let Some(dir) = self.file.parent() {
            fs::create_dir_all(dir).map_err(error)?;
        }
        let temp = self.file.with_file_name(TEMP_FILE);
        fs::write(&temp, encrypt(key, contents)?).map_err(error)?;
        fs::rename(&temp, &self.file).map_err(error)
    }
}

// ── Public API ────────────────────────────
impl Vault {
    /// The vault in `dir`. Also removes cookies files a crash left behind.
    pub fn open(dir: &Path, source: KeySource) -> Self {
        remove_leftover_files();
        Self { inner: Arc::new(Inner { file: dir.join(FILE), source, key: Mutex::new(None) }) }
    }

    /// The remembered sites, by name.
    pub fn sites(&self) -> Result<Vec<RememberedSite>, String> {
        let mut key = self.inner.key.lock().unwrap_or_else(PoisonError::into_inner);
        let contents = self.inner.read(&mut key)?;
        Ok(contents
            .sites
            .into_iter()
            .map(|(site, saved)| RememberedSite {
                site,
                cookies: u32::try_from(saved.cookies.len()).unwrap_or(u32::MAX),
                saved_at: saved.saved_at,
            })
            .collect())
    }

    /// The remembered cookies for `site`; `None` when there are none or they can't be read.
    pub fn cookies_for(&self, site: &str) -> Option<Vec<Cookie>> {
        let mut key = self.inner.key.lock().unwrap_or_else(PoisonError::into_inner);
        match self.inner.read(&mut key) {
            Ok(mut contents) => contents.sites.remove(site).map(|saved| saved.cookies),
            Err(error) => {
                tracing::warn!("{error}");
                None
            }
        }
    }

    /// Remembers `cookies` (already checked) for `site`, replacing what was there.
    pub fn remember(&self, site: &str, cookies: Vec<Cookie>) -> Result<(), String> {
        let mut key = self.inner.key.lock().unwrap_or_else(PoisonError::into_inner);
        let mut contents = self.inner.read(&mut key)?;
        contents.sites.insert(site.to_string(), Saved { saved_at: now(), cookies });
        self.inner.write(&mut key, &contents)
    }

    /// Forgets `site`'s cookies.
    pub fn forget(&self, site: &str) -> Result<(), String> {
        let mut key = self.inner.key.lock().unwrap_or_else(PoisonError::into_inner);
        let mut contents = self.inner.read(&mut key)?;
        if contents.sites.remove(site).is_some() {
            self.inner.write(&mut key, &contents)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{FILE, KeySource, Vault};
    use crate::vault::cookie::Cookie;
    use std::fs;

    fn cookie(name: &str, value: &str) -> Cookie {
        Cookie {
            domain: ".youtube.com".into(),
            name: name.into(),
            value: value.into(),
            path: "/".into(),
            secure: true,
            http_only: true,
            host_only: false,
            expires: None,
        }
    }

    #[test]
    fn remembers_and_forgets_a_site() {
        let dir = tempfile::tempdir().unwrap();
        let vault = Vault::open(dir.path(), KeySource::Fixed([7; 32]));
        assert!(vault.sites().unwrap().is_empty());
        assert!(!dir.path().join(FILE).exists());

        vault.remember("youtube.com", vec![cookie("SID", "secret-sid")]).unwrap();
        vault.remember("x.com", vec![cookie("auth_token", "a"), cookie("ct0", "b")]).unwrap();
        let sites: Vec<_> = vault.sites().unwrap().into_iter().map(|s| (s.site, s.cookies)).collect();
        assert_eq!(sites, [("x.com".to_string(), 2), ("youtube.com".to_string(), 1)]);

        // A fresh handle reads what the first one wrote.
        let again = Vault::open(dir.path(), KeySource::Fixed([7; 32]));
        assert_eq!(again.cookies_for("youtube.com").unwrap()[0].value, "secret-sid");
        again.forget("youtube.com").unwrap();
        assert!(again.cookies_for("youtube.com").is_none());
        assert_eq!(again.sites().unwrap().len(), 1);
    }

    #[test]
    fn keeps_the_file_encrypted() {
        let dir = tempfile::tempdir().unwrap();
        let vault = Vault::open(dir.path(), KeySource::Fixed([7; 32]));
        vault.remember("youtube.com", vec![cookie("SID", "secret-sid")]).unwrap();
        let bytes = fs::read(dir.path().join(FILE)).unwrap();
        assert!(bytes.starts_with(b"SCV1"));
        assert!(!String::from_utf8_lossy(&bytes).contains("secret-sid"));
        assert!(!String::from_utf8_lossy(&bytes).contains("youtube"));

        let wrong_key = Vault::open(dir.path(), KeySource::Fixed([8; 32]));
        assert!(wrong_key.sites().unwrap_err().contains("can't be read"));
        assert!(wrong_key.cookies_for("youtube.com").is_none());
    }
}

//! Scan: duplicate removal over the library as a job. Each picture in the catalog is hashed once (the hash is kept in
//! the catalog), then grouped; the window takes the groups when the job ends. Copies the user lets go move to the
//! system trash and leave the catalog; nothing is deleted outright (ADR-0009).

// ── Imports ───────────────────────────────
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};

use serde::Deserialize;
use ts_rs::TS;

use super::fingerprint::{fingerprint, jpeg_quality};
use super::group::{Picture, Strictness, groups};
use crate::jobs::{Job, JobId, Jobs};
use crate::library;

// ── Types ─────────────────────────────────
/// What to look through, and how alike copies must be.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DuplicateRequest {
    /// A folder relative to the library root; `None` for the whole library.
    pub folder: Option<String>,
    pub strictness: Strictness,
}

/// Finished scans' groups, until the window takes them.
#[derive(Clone, Default)]
pub struct Duplicates {
    found: Arc<Mutex<HashMap<JobId, Vec<Vec<Picture>>>>>,
}

// ── Private helpers ───────────────────────
/// Hashes what isn't hashed yet and groups everything. Blocking: decoding pictures is CPU work.
fn find(root: &std::path::Path, request: &DuplicateRequest, job: &Job) -> Result<Vec<Vec<Picture>>, String> {
    let rows = library::images(root, request.folder.as_deref())?;
    let total = rows.len() as u64;
    let mut pictures = Vec::with_capacity(rows.len());
    for (index, row) in rows.into_iter().enumerate() {
        if job.cancelled() {
            return Err("Cancelled.".into());
        }
        let file = root.join(&row.path);
        let name = row.path.rsplit('/').next().unwrap_or(&row.path).to_string();
        job.items(index as u64, Some(total), Some(name.clone()));
        if !file.is_file() {
            continue;
        }
        let known = match row.fingerprint {
            Some(known) => known,
            None => match fingerprint(&file) {
                Ok(made) => {
                    library::set_fingerprint(root, &row.path, made)?;
                    made
                }
                Err(error) => {
                    tracing::warn!("Skipped a picture duplicate removal can't read: {error}");
                    continue;
                }
            },
        };
        let format = row.path.rsplit_once('.').map(|(_, ext)| ext.to_ascii_uppercase()).unwrap_or_default();
        let (hash, width, height) = known;
        let quality = jpeg_quality(&file);
        pictures.push(Picture { path: row.path, name, width, height, bytes: row.bytes, format, quality, hash });
    }
    job.items(total, Some(total), None);
    Ok(groups(pictures, request.strictness))
}

// ── Public API ────────────────────────────
impl Duplicates {
    /// Starts a scan of the library at `root` as a job and returns its id; take the groups with `take` once it ends.
    /// Must be called inside a Tokio runtime.
    pub fn start(&self, jobs: &Arc<Jobs>, root: PathBuf, request: DuplicateRequest) -> JobId {
        let found = self.found.clone();
        jobs.start(move |job| async move {
            let id = job.id();
            let groups = tokio::task::spawn_blocking(move || find(&root, &request, &job))
                .await
                .map_err(|e| format!("The scan stopped: {e}"))??;
            tracing::info!("Found {} groups of duplicate pictures", groups.len());
            found.lock().unwrap_or_else(PoisonError::into_inner).insert(id, groups);
            Ok(())
        })
    }

    /// The groups a finished scan found, best copy first in each; once.
    pub fn take(&self, id: JobId) -> Option<Vec<Vec<Picture>>> {
        self.found.lock().unwrap_or_else(PoisonError::into_inner).remove(&id)
    }
}

/// Moves the library files at `paths` to the system trash and drops them from the catalog. Returns how many moved;
/// one that can't move stops the rest, and those already moved stay forgotten.
pub fn trash(root: &std::path::Path, paths: &[String]) -> Result<u32, String> {
    let mut moved = Vec::new();
    let mut failed = None;
    for path in paths {
        let result = library::locate(root, path)
            .and_then(|file| ::trash::delete(&file).map_err(|e| format!("Can't move {path} to the trash: {e}")));
        match result {
            Ok(()) => moved.push(path.clone()),
            Err(error) => {
                failed = Some(error);
                break;
            }
        }
    }
    library::forget(root, &moved)?;
    tracing::info!("Moved {} duplicate pictures to the trash", moved.len());
    match failed {
        Some(error) => Err(error),
        None => Ok(u32::try_from(moved.len()).unwrap_or(u32::MAX)),
    }
}

#[cfg(test)]
mod tests {
    use super::{DuplicateRequest, Duplicates};
    use crate::dedupe::Strictness;
    use crate::jobs::{JobEvent, JobResult, Jobs};
    use crate::library::{self, Record};
    use image::{ImageFormat, Rgb, RgbImage};
    use std::fs;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    /// Soft rings, or with `stripes` a different picture: bold diagonal bands.
    fn picture(stripes: bool) -> RgbImage {
        RgbImage::from_fn(320, 240, |x, y| {
            if stripes {
                let band = if (x + y) / 40 % 2 == 0 { 230 } else { 20 };
                return Rgb([band, band, band]);
            }
            let ring = ((x as f32 - 200.0).hypot(y as f32 - 90.0) / 14.0).sin().abs();
            Rgb([(x * 255 / 320) as u8, (ring * 255.0) as u8, (y * 255 / 240) as u8])
        })
    }

    fn add(root: &std::path::Path, path: &str, image: &RgbImage, format: ImageFormat) {
        let file = root.join(path);
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        image.save_with_format(&file, format).unwrap();
        let size = fs::metadata(&file).unwrap().len();
        let record = Record {
            path: path.into(),
            kind: "image",
            source_url: "https://example.com".into(),
            site: None,
            user: None,
            size_bytes: size,
            duration_secs: None,
            codec: None,
            transferred_bytes: size,
        };
        library::record(root, &record).unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn finds_copies_and_keeps_their_hashes() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().to_path_buf();
        fs::create_dir_all(root.join(".scrappy")).unwrap();
        add(&root, "Images/a/cat.png", &picture(false), ImageFormat::Png);
        add(&root, "Images/b/cat.jpg", &picture(false), ImageFormat::Jpeg);
        add(&root, "Images/a/other.png", &picture(true), ImageFormat::Png);

        let ended = Arc::new(Mutex::new(Vec::new()));
        let sink = ended.clone();
        let jobs = Jobs::new(Arc::new(move |event| {
            if let JobEvent::Finished(end) = event {
                sink.lock().unwrap().push(end.result);
            }
        }));
        let duplicates = Duplicates::default();
        let request = DuplicateRequest { folder: None, strictness: Strictness::Normal };
        let id = duplicates.start(&jobs, root.clone(), request);
        while ended.lock().unwrap().is_empty() {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        assert_eq!(ended.lock().unwrap()[0], JobResult::Done);
        let groups = duplicates.take(id).unwrap();
        let names: Vec<Vec<&str>> = groups.iter().map(|g| g.iter().map(|p| p.path.as_str()).collect()).collect();
        assert_eq!(names, [["Images/a/cat.png", "Images/b/cat.jpg"]]);
        assert!(groups[0][1].quality.is_some());
        assert!(duplicates.take(id).is_none());
        assert!(library::images(&root, None).unwrap().iter().all(|row| row.fingerprint.is_some()));
    }

    #[test]
    fn refuses_paths_outside_the_library() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join(".scrappy")).unwrap();
        assert!(super::trash(dir.path(), &["../secret.png".to_string()]).is_err());
        assert_eq!(super::trash(dir.path(), &[]).unwrap(), 0);
    }
}

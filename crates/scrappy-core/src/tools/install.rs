//! Install: downloads a tool's pinned file with progress, checks its SHA-256 while it arrives, and unpacks only the
//! tool's programs into the tools folder (ADR-0012). A file that fails the check is deleted, never unpacked.

// ── Imports ───────────────────────────────
use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;

use super::manifest::{self, Archive, Tool};
use crate::net;
use crate::jobs::{Job, JobPhase};

// ── Constants ─────────────────────────────
/// Downloads land here first and are deleted after unpacking.
const PARTIAL_DIR: &str = ".partial";

// ── Private helpers ───────────────────────
/// The file name a program has on this system.
pub(super) fn program_file(program: &str) -> String {
    format!("{program}{}", std::env::consts::EXE_SUFFIX)
}

/// Downloads `url` to `file`, reporting progress, and returns the file's SHA-256 in lowercase hex.
async fn fetch(url: &str, file: &Path, job: &Job) -> Result<String, String> {
    let failed = |e: reqwest::Error| format!("Download failed: {e}");
    let client = net::client().build().map_err(failed)?;
    let mut response = client.get(url).send().await.and_then(|r| r.error_for_status()).map_err(failed)?;
    let total = response.content_length();
    let mut out = tokio::fs::File::create(file).await.map_err(|e| format!("Can't save the download: {e}"))?;
    let mut hasher = Sha256::new();
    let mut done = 0;
    while let Some(chunk) = response.chunk().await.map_err(failed)? {
        if job.cancelled() {
            return Err("Cancelled.".into());
        }
        hasher.update(&chunk);
        out.write_all(&chunk).await.map_err(|e| format!("Can't save the download: {e}"))?;
        done += chunk.len() as u64;
        job.progress(JobPhase::Download, done, total);
    }
    out.flush().await.map_err(|e| format!("Can't save the download: {e}"))?;
    Ok(format!("{:x}", hasher.finalize()))
}

/// Writes `reader` to `dir/<name>` through a temporary file, so a half-written program never replaces a working one.
fn place(mut reader: impl io::Read, dir: &Path, name: &str) -> Result<(), String> {
    let target = dir.join(name);
    let temporary = dir.join(format!("{name}.new"));
    let failed = |e: io::Error| format!("Can't install {name}: {e}");
    io::copy(&mut reader, &mut File::create(&temporary).map_err(failed)?).map_err(failed)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&temporary, fs::Permissions::from_mode(0o755)).map_err(failed)?;
    }
    fs::rename(&temporary, &target).map_err(failed)
}

/// The last part of an archive entry's path, which is all Scrappy matches on: entries can't pick where they land.
fn entry_name(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

/// Puts `programs` from the downloaded `file` into `dir`, and nothing else from it.
pub(super) fn unpack(file: &Path, archive: Archive, programs: &[&str], dir: &Path) -> Result<(), String> {
    let wanted: Vec<String> = programs.iter().map(|program| program_file(program)).collect();
    let open = || File::open(file).map_err(|e| format!("Can't open the download: {e}"));
    let broken = |e: &dyn std::fmt::Display| format!("The download is damaged: {e}");
    let mut placed = Vec::new();
    match archive {
        Archive::Plain => {
            place(open()?, dir, &wanted[0])?;
            placed.push(wanted[0].clone());
        }
        Archive::Zip => {
            let mut zip = zip::ZipArchive::new(open()?).map_err(|e| broken(&e))?;
            for index in 0..zip.len() {
                let entry = zip.by_index(index).map_err(|e| broken(&e))?;
                let name = entry_name(&entry.name().map_err(|e| broken(&e))?).to_string();
                if entry.is_file() && wanted.contains(&name) && !placed.contains(&name) {
                    place(entry, dir, &name)?;
                    placed.push(name);
                }
            }
        }
        Archive::TarXz => {
            let mut tar = tar::Archive::new(liblzma::read::XzDecoder::new(open()?));
            for entry in tar.entries().map_err(|e| broken(&e))? {
                let entry = entry.map_err(|e| broken(&e))?;
                let path = entry.path().map_err(|e| broken(&e))?.to_string_lossy().into_owned();
                let name = entry_name(&path).to_string();
                if entry.header().entry_type().is_file() && wanted.contains(&name) && !placed.contains(&name) {
                    place(entry, dir, &name)?;
                    placed.push(name);
                }
            }
        }
    }
    match wanted.iter().find(|name| !placed.contains(name)) {
        Some(missing) => Err(format!("The download has no {missing}.")),
        None => Ok(()),
    }
}

// ── Public API ────────────────────────────
/// Downloads, checks and unpacks `tool` into `dir`. The download is deleted whatever happens.
pub(super) async fn install(dir: &Path, tool: Tool, job: &Job) -> Result<(), String> {
    let download = manifest::download_for(tool).ok_or("There's no download of this tool for this system.")?;
    let partial: PathBuf = dir.join(PARTIAL_DIR);
    fs::create_dir_all(&partial).map_err(|e| format!("Can't create the tools folder: {e}"))?;
    let file = partial.join(entry_name(download.url));

    let result = async {
        let sha256 = fetch(download.url, &file, job).await?;
        if sha256 != download.sha256 {
            tracing::warn!("{tool:?} download didn't match its pinned SHA-256; deleted");
            return Err("The download didn't match its checksum, so it was deleted.".to_string());
        }
        job.progress(JobPhase::Unpack, 0, None);
        let (file, dir, archive) = (file.clone(), dir.to_path_buf(), download.archive);
        tokio::task::spawn_blocking(move || unpack(&file, archive, tool.programs(), &dir))
            .await
            .map_err(|e| format!("Unpacking stopped: {e}"))?
    }
    .await;

    let _ = fs::remove_file(&file);
    if result.is_ok() {
        tracing::info!("{tool:?} installed");
    }
    result
}

#[cfg(test)]
mod tests {
    use super::{Archive, entry_name, program_file, unpack};
    use std::fs::{self, File};
    use std::io::Write;

    #[test]
    fn matches_on_the_last_part_only() {
        assert_eq!(entry_name("ffmpeg-n9/bin/ffmpeg.exe"), "ffmpeg.exe");
        assert_eq!(entry_name("../../evil/ffmpeg"), "ffmpeg");
        assert_eq!(entry_name("deno"), "deno");
    }

    #[test]
    fn unpacks_only_the_programs_from_a_zip() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("ffmpeg.zip");
        let mut zip = zip::ZipWriter::new(File::create(&file).unwrap());
        let options = zip::write::SimpleFileOptions::default();
        for (name, body) in [("x/bin/ffmpeg", "a"), ("x/bin/ffprobe", "b"), ("x/bin/ffplay", "c"), ("x/README", "d")] {
            let name = name.replace("ffmpeg", &program_file("ffmpeg")).replace("ffprobe", &program_file("ffprobe"));
            zip.start_file(name, options).unwrap();
            zip.write_all(body.as_bytes()).unwrap();
        }
        zip.finish().unwrap();

        unpack(&file, Archive::Zip, &["ffmpeg", "ffprobe"], dir.path()).unwrap();
        assert_eq!(fs::read_to_string(dir.path().join(program_file("ffprobe"))).unwrap(), "b");
        assert!(!dir.path().join("README").exists() && !dir.path().join("x").exists());
    }

    #[test]
    fn unpacks_a_tar_xz_and_a_plain_file() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("deno.tar.xz");
        let mut tar = tar::Builder::new(liblzma::write::XzEncoder::new(File::create(&file).unwrap(), 6));
        let body = b"deno program";
        let mut header = tar::Header::new_gnu();
        header.set_size(body.len() as u64);
        header.set_mode(0o755);
        header.set_cksum();
        tar.append_data(&mut header, format!("deno-x/{}", program_file("deno")), &body[..]).unwrap();
        tar.into_inner().unwrap().finish().unwrap();
        unpack(&file, Archive::TarXz, &["deno"], dir.path()).unwrap();
        assert_eq!(fs::read(dir.path().join(program_file("deno"))).unwrap(), body);

        let plain = dir.path().join("download.bin");
        fs::write(&plain, "yt-dlp program").unwrap();
        unpack(&plain, Archive::Plain, &["yt-dlp"], dir.path()).unwrap();
        assert_eq!(fs::read_to_string(dir.path().join(program_file("yt-dlp"))).unwrap(), "yt-dlp program");
    }

    #[test]
    fn says_what_an_archive_is_missing() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("empty.zip");
        zip::ZipWriter::new(File::create(&file).unwrap()).finish().unwrap();
        let error = unpack(&file, Archive::Zip, &["ffmpeg"], dir.path()).unwrap_err();
        assert!(error.starts_with("The download has no ffmpeg"), "{error}");
    }
}

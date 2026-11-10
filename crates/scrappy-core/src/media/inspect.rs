//! Inspect: ffprobe reads what a download holds (codecs and length), and FFmpeg re-encodes it to a preset with
//! progress (ADR-0003).

// ── Imports ───────────────────────────────
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use serde::Deserialize;
use tokio::io::{AsyncBufReadExt, BufReader};

use super::engine;
use crate::jobs::{Job, JobPhase};
use crate::tools;

// ── Constants ─────────────────────────────
const INSPECT_TIMEOUT: Duration = Duration::from_secs(60);
const CANCEL_CHECK: Duration = Duration::from_millis(200);

// ── Types ─────────────────────────────────
/// What ffprobe found in a file.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct MediaInfo {
    pub video: Option<String>,
    pub audio: Option<String>,
    /// Seconds.
    pub duration: Option<f64>,
}

#[derive(Deserialize)]
struct Probed {
    #[serde(default)]
    streams: Vec<Stream>,
    format: Option<Container>,
}

#[derive(Deserialize)]
struct Stream {
    codec_type: Option<String>,
    codec_name: Option<String>,
}

#[derive(Deserialize)]
struct Container {
    duration: Option<String>,
}

// ── Private helpers ───────────────────────
fn parse(json: &str) -> Result<MediaInfo, String> {
    let probed: Probed = serde_json::from_str(json).map_err(|e| format!("ffprobe's answer couldn't be read: {e}"))?;
    let codec = |kind: &str| {
        probed.streams.iter().find(|s| s.codec_type.as_deref() == Some(kind)).and_then(|s| s.codec_name.clone())
    };
    Ok(MediaInfo {
        video: codec("video"),
        audio: codec("audio"),
        duration: probed.format.and_then(|f| f.duration).and_then(|d| d.parse().ok()),
    })
}

// ── Public API ────────────────────────────
/// The codecs and length of `file`.
pub(super) async fn inspect(ffprobe: &Path, file: &Path) -> Result<MediaInfo, String> {
    let mut command = tools::command(ffprobe);
    command.args(["-v", "error", "-show_entries", "format=duration:stream=codec_type,codec_name", "-of", "json"]);
    let output = tokio::time::timeout(INSPECT_TIMEOUT, command.arg(file).output())
        .await
        .map_err(|_| "ffprobe took too long.".to_string())?
        .map_err(|e| format!("ffprobe didn't start: {e}"))?;
    if !output.status.success() {
        return Err(format!("ffprobe couldn't read the download: {}", String::from_utf8_lossy(&output.stderr).trim()));
    }
    parse(&String::from_utf8_lossy(&output.stdout))
}

/// Re-encodes `input` into `output` with `args`, reporting progress against `duration`. A cancelled or failed encode
/// leaves no output behind.
pub(super) async fn encode(
    ffmpeg: &Path,
    input: &Path,
    output: &Path,
    args: &[&str],
    duration: Option<f64>,
    job: &Job,
) -> Result<(), String> {
    let mut command = tools::command(ffmpeg);
    command.args(["-hide_banner", "-loglevel", "error", "-nostats", "-progress", "pipe:1", "-y", "-i"]);
    command.arg(input).args(args).arg(output).stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = command.spawn().map_err(|e| format!("FFmpeg didn't start: {e}"))?;
    let stderr = child.stderr.take().map(|stderr| {
        tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            let mut kept = Vec::new();
            while let Ok(Some(line)) = lines.next_line().await {
                kept.push(line);
            }
            kept
        })
    });
    let total = duration.map(|seconds| (seconds * 1_000_000.0) as u64);
    let mut lines = BufReader::new(child.stdout.take().ok_or("FFmpeg gave no output.")?).lines();
    let result = loop {
        tokio::select! {
            line = lines.next_line() => match line {
                Ok(Some(line)) => {
                    if let Some(done) = line.strip_prefix("out_time_us=").and_then(|us| us.parse::<u64>().ok()) {
                        job.progress(JobPhase::Encode, total.map_or(done, |total| done.min(total)), total);
                    }
                }
                _ => break Ok(()),
            },
            () = tokio::time::sleep(CANCEL_CHECK) => if job.cancelled() { break Err("Cancelled.".to_string()) },
        }
    };
    let status = match result {
        Ok(()) => child.wait().await.map_err(|e| format!("FFmpeg stopped: {e}")),
        Err(cancelled) => {
            let _ = child.kill().await;
            Err(cancelled)
        }
    };
    let errors = match stderr {
        Some(task) => task.await.unwrap_or_default(),
        None => Vec::new(),
    };
    match status {
        Ok(status) if status.success() => Ok(()),
        other => {
            let _ = std::fs::remove_file(output);
            Err(other.err().unwrap_or_else(|| format!("FFmpeg couldn't convert it: {}", engine::error_text(&errors))))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::parse;

    #[test]
    fn reads_codecs_and_length() {
        let json = r#"{"streams":[{"codec_type":"video","codec_name":"h264"},{"codec_type":"audio","codec_name":"aac"}],
            "format":{"duration":"18.947483"}}"#;
        let info = parse(json).unwrap();
        assert_eq!((info.video.as_deref(), info.audio.as_deref()), (Some("h264"), Some("aac")));
        assert_eq!(info.duration, Some(18.947483));
        assert_eq!(parse(r#"{"streams":[{"codec_type":"audio","codec_name":"mp3"}]}"#).unwrap().video, None);
    }
}

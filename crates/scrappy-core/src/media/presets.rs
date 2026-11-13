//! Presets: ADR-0003's output presets as data: how yt-dlp picks and merges streams, and how FFmpeg re-encodes what
//! doesn't fit. Presets are data, so nothing branches on a preset's id. Pure data table: exempt from the size limit.

// ── Imports ───────────────────────────────
use serde::Serialize;
use ts_rs::TS;

// ── Types ─────────────────────────────────
/// A preset, for the window's picker.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PresetInfo {
    pub id: String,
    pub name: String,
    /// Audio only: no quality to pick.
    pub audio: bool,
}

/// An output preset.
pub(super) struct Preset {
    pub id: &'static str,
    pub name: &'static str,
    pub audio: bool,
    /// yt-dlp's arguments after the format choice: sort order, merge container, audio extraction.
    pub yt_dlp: &'static [&'static str],
    /// When the download's codecs or container don't fit, FFmpeg re-encodes it.
    pub encode: Option<Encode>,
}

/// What a preset's file must hold, and how FFmpeg gets it there.
pub(super) struct Encode {
    /// ffprobe's codec names that already fit; such a stream is copied, not re-encoded.
    pub video_ok: &'static [&'static str],
    pub video: &'static [&'static str],
    pub audio_ok: &'static [&'static str],
    pub audio: &'static [&'static str],
    /// Arguments for the output whatever is re-encoded.
    pub extra: &'static [&'static str],
    /// The output's extension.
    pub ext: &'static str,
}

// ── Constants ─────────────────────────────
const AAC: &[&str] = &["-c:a", "aac", "-b:a", "192k"];

pub(super) const PRESETS: &[Preset] = &[
    Preset {
        id: "editor-mp4",
        name: "MP4 (H.264)",
        audio: false,
        // Highest resolution first, then H.264 and AAC among equals; 4K (VP9 or AV1 on YouTube) is re-encoded.
        yt_dlp: &["-S", "res,fps,vcodec:h264,acodec:aac", "--merge-output-format", "mp4"],
        encode: Some(Encode {
            video_ok: &["h264"],
            video: &["-c:v", "libx264", "-preset", "medium", "-crf", "18", "-pix_fmt", "yuv420p"],
            audio_ok: &["aac"],
            audio: AAC,
            extra: &["-movflags", "+faststart"],
            ext: "mp4",
        }),
    },
    Preset {
        id: "original",
        name: "MKV (original)",
        audio: false,
        yt_dlp: &["--merge-output-format", "mkv"],
        encode: None,
    },
    Preset {
        id: "hevc-mp4",
        name: "MP4 (HEVC)",
        audio: false,
        yt_dlp: &["-S", "res,fps,vcodec:hevc,acodec:aac", "--merge-output-format", "mkv"],
        encode: Some(Encode {
            video_ok: &["hevc"],
            video: &["-c:v", "libx265", "-preset", "medium", "-crf", "22", "-pix_fmt", "yuv420p"],
            audio_ok: &["aac"],
            audio: AAC,
            // `hvc1` is the tag Apple's players and editors need.
            extra: &["-tag:v", "hvc1", "-movflags", "+faststart"],
            ext: "mp4",
        }),
    },
    Preset {
        id: "prores-mov",
        name: "MOV (ProRes)",
        audio: false,
        yt_dlp: &["--merge-output-format", "mkv"],
        encode: Some(Encode {
            video_ok: &["prores"],
            video: &["-c:v", "prores_ks", "-profile:v", "2", "-pix_fmt", "yuv422p10le"],
            audio_ok: &["pcm_s16le"],
            audio: &["-c:a", "pcm_s16le"],
            extra: &[],
            ext: "mov",
        }),
    },
    Preset {
        id: "mp3",
        name: "MP3 (320 kbps)",
        audio: true,
        yt_dlp: &["-x", "--audio-format", "mp3", "--audio-quality", "320K"],
        encode: None,
    },
    Preset { id: "m4a", name: "M4A (AAC)", audio: true, yt_dlp: &["-x", "--audio-format", "m4a"], encode: None },
    Preset { id: "flac", name: "FLAC", audio: true, yt_dlp: &["-x", "--audio-format", "flac"], encode: None },
    Preset { id: "wav", name: "WAV", audio: true, yt_dlp: &["-x", "--audio-format", "wav"], encode: None },
];

// ── Public API ────────────────────────────
/// Every preset, the default first.
pub fn presets() -> Vec<PresetInfo> {
    PRESETS
        .iter()
        .map(|p| PresetInfo { id: p.id.to_string(), name: p.name.to_string(), audio: p.audio })
        .collect()
}

/// The preset with `id`.
pub(super) fn find(id: &str) -> Option<&'static Preset> {
    PRESETS.iter().find(|preset| preset.id == id)
}

/// The preset a send from the extension takes (ADR-0035): the start preset from Settings (`start`, by id; the first
/// when unknown), or for a link with no video, that one when it's audio, else the first audio preset.
pub(super) fn for_send(start: Option<&str>, has_video: bool) -> &'static Preset {
    let start = start.and_then(find).unwrap_or(&PRESETS[0]);
    if has_video || start.audio {
        return start;
    }
    PRESETS.iter().find(|preset| preset.audio).unwrap_or(start)
}

impl Preset {
    /// yt-dlp's format choice: the best streams, capped at `max_height` for video. Falls back to the best there is
    /// when nothing is that small.
    pub(super) fn format(&self, max_height: Option<u32>) -> String {
        match (self.audio, max_height) {
            (true, _) => "ba/b".into(),
            (false, Some(h)) => format!("bv*[height<={h}]+ba/b[height<={h}]/bv*+ba/b"),
            (false, None) => "bv*+ba/b".into(),
        }
    }
}

impl Encode {
    /// FFmpeg's arguments to turn a file with these codecs and extension into this preset's output, or `None` when
    /// it already fits. Streams that fit are copied.
    pub(super) fn args(&self, video: Option<&str>, audio: Option<&str>, ext: &str) -> Option<Vec<&'static str>> {
        let video_fits = video.is_none_or(|codec| self.video_ok.contains(&codec));
        let audio_fits = audio.is_none_or(|codec| self.audio_ok.contains(&codec));
        if video_fits && audio_fits && ext.eq_ignore_ascii_case(self.ext) {
            return None;
        }
        let mut args = Vec::new();
        args.extend_from_slice(if video_fits { &["-c:v", "copy"] } else { self.video });
        args.extend_from_slice(if audio_fits { &["-c:a", "copy"] } else { self.audio });
        args.extend_from_slice(self.extra);
        Some(args)
    }
}

#[cfg(test)]
mod tests {
    use super::{PRESETS, find, for_send, presets};

    #[test]
    fn a_send_takes_the_start_preset_or_an_audio_one() {
        assert_eq!(for_send(None, true).id, "editor-mp4");
        assert_eq!(for_send(Some("hevc-mp4"), true).id, "hevc-mp4");
        assert_eq!(for_send(Some("gone"), true).id, "editor-mp4");
        assert_eq!(for_send(Some("hevc-mp4"), false).id, "mp3");
        assert_eq!(for_send(Some("flac"), false).id, "flac");
        assert_eq!(for_send(Some("flac"), true).id, "flac");
    }

    #[test]
    fn ids_are_unique_and_the_default_comes_first() {
        let ids: Vec<String> = presets().into_iter().map(|p| p.id).collect();
        assert_eq!(ids[0], "editor-mp4");
        let mut unique = ids.clone();
        unique.dedup();
        assert_eq!(unique.len(), PRESETS.len());
    }

    #[test]
    fn caps_the_height_for_video_only() {
        let editor = find("editor-mp4").unwrap();
        assert_eq!(editor.format(Some(1080)), "bv*[height<=1080]+ba/b[height<=1080]/bv*+ba/b");
        assert_eq!(find("mp3").unwrap().format(Some(1080)), "ba/b");
    }

    #[test]
    fn re_encodes_only_what_does_not_fit() {
        let encode = find("editor-mp4").unwrap().encode.as_ref().unwrap();
        assert_eq!(encode.args(Some("h264"), Some("aac"), "mp4"), None);
        let vp9 = encode.args(Some("vp9"), Some("aac"), "mp4").unwrap();
        assert!(vp9.starts_with(&["-c:v", "libx264"]) && vp9.contains(&"copy"));
        let hevc = find("hevc-mp4").unwrap().encode.as_ref().unwrap();
        let remux = hevc.args(Some("hevc"), Some("aac"), "mkv").unwrap();
        assert_eq!(&remux[..4], &["-c:v", "copy", "-c:a", "copy"]);
        assert!(remux.contains(&"hvc1"));
    }
}

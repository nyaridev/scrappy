# ADR-0003: Editor-safe encoding presets

**Status:** Accepted
**Date:** 2026-11-10

## Context

YouTube's best streams are VP9 or AV1 video with Opus audio, often in WebM. Premiere Pro, Final Cut and Vegas don't
import those. Audio-only downloads ending as `.webm`/Opus were the old app's main complaint, and `.m4a` (AAC) fails in
DaVinci Resolve on Linux, which has no AAC at all. YouTube only offers H.264 up to 1080p.

## Decision

Every download uses an output preset from `scrappy-core::media`. Presets are data (name, yt-dlp format sort,
container, optional ffmpeg encode), shipped as defaults and editable in Settings.

| Preset | What |
|---|---|
| MP4 (H.264) (default) | H.264 + AAC in MP4 (yt-dlp's `-t mp4`). Sources above 1080p or not in H.264 are re-encoded with `libx264 -crf 18`, or the GPU encoder when there is one (NVENC, QSV, AMF, VideoToolbox) |
| MKV (original) | The best streams, no re-encode, in MKV |
| MP4 (HEVC) | H.265 with the `hvc1` tag: 4K that stays small |
| MOV (ProRes) | ProRes 422 or DNxHR HQ in MOV with PCM audio (the choice for Resolve on Linux) |
| Audio | MP3 320 (default), AAC, FLAC or WAV. Opus is kept only as "original, not editor-safe" |

After each download ffprobe reads the result, and the UI shows its codec and container.

## Options considered

- **Always re-encode to H.264:** safe, but slow at 4K and wasteful when the source is already H.264.
- **Never re-encode:** fast, but brings back the files editors can't open.

## Consequences

- **Easier:** files open in editors by default, and anyone can add a preset without code.
- **Harder:** editor-safe 1440p and 4K downloads take encode time, and GPU encoders must be detected on each machine.
- **Later:** a new preset is one entry in the defaults; a default preset per site is a setting.
- **Rules:** `.claude/rules/architecture/core.md`.

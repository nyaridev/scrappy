# ADR-0002: External engines as managed tools

**Status:** Accepted
**Date:** 2026-11-10

## Context

There's no maintained pure-Rust YouTube downloader in 2026: `rusty_ytdl` stopped in 2024 and `rustypipe` trails
yt-dlp. YouTube changes things almost monthly (SABR streaming, PO tokens, signature changes), and yt-dlp now needs a
JavaScript runtime (Deno) for YouTube. gallery-dl covers X, Instagram, Facebook, Pinterest and many galleries. ffmpeg
and ffprobe do the encoding and the checks. The old Python `video-downloader` used yt-dlp as a library and downloaded
Deno and ffmpeg on demand; Rust can't load yt-dlp as a library.

## Decision

`scrappy-core::tools` manages five external programs: `yt-dlp`, `deno`, `gallery-dl`, `ffmpeg` and `ffprobe`.

- They're downloaded on first run, with progress, into the app data folder and checked against a pinned SHA256
  manifest. Each one can point at a copy on `PATH` instead.
- Core runs them as separate processes (`tokio::process`, no console window on Windows). The window never gets a
  shell permission.
- yt-dlp updates itself in the background (`--update-to nightly`), so YouTube fixes arrive without a Scrappy release.
- Rust reads only their machine-readable output: `-J` to probe, `--newline --progress-template` with JSON for
  progress, gallery-dl's `-j`.
- Sources: yt-dlp's release binaries, Deno releases, gallery-dl releases (Windows, Linux) and BtbN ffmpeg builds
  (Windows, Linux). macOS ffmpeg and gallery-dl are built in our CI (M14).

## Options considered

- **Pure Rust (`rustypipe`, `rusty_ytdl`):** breaks with YouTube, lags yt-dlp, and covers YouTube only.
- **The `yt-dlp` crate:** GPL, heavy (caches, webhooks), and hides the arguments and progress we need to control.
- **Bundle everything in the installer:** works offline, but adds about 150 MB and ties tool updates to app releases.

## Consequences

- **Easier:** site support comes from two very active projects, and YouTube breakage is fixed by updating yt-dlp.
- **Harder:** the first launch needs a download. The tools are GPL builds, so they stay separate processes and their
  licences ship in `THIRD_PARTY_LICENSES`.
- **Later:** a new engine is one more entry in the tools manifest and a driver in `media/`.
- **Rules:** `.claude/rules/architecture/core.md`, `security.md`.

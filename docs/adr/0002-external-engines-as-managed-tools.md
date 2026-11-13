# ADR-0002: External engines as managed tools

**Status:** Accepted; its sources and checksum timing are updated by ADR-0012; the window's probe lists as it comes
(ADR-0030)
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
  progress, gallery-dl's `-J`.
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

## Implementation notes (M7)

- `media/route.rs` picks the engine: a list of sites gallery-dl takes (X, Instagram, Bluesky, Pixiv, DeviantArt,
  Tumblr, Pinterest, Facebook photos), every other link to yt-dlp. A gallery-dl site's videos go to gallery-dl too,
  so a post with photos and a video is one job.
- gallery-dl runs with `--config-ignore` and its options on the command line (`-d`, `--download-archive`,
  `--cookies`, `--range`) instead of a config file written per run. It prints each saved file's path, and `# path`
  for one its archive skipped. Its layout `Social/<site>/<user>/` is ADR-0004's.
- Every yt-dlp run passes `--no-playlist`; the probe adds `--yes-playlist` for a video link that names a playlist
  (`watch?v=…&list=PL…`), a YouTube Mix (`list=RD…`) included. Such a list's probe carries the video's own link (the
  link without `list`, `index`, `start_radio`, `pp`), and the Download view's "Just this video" checks that instead.
- Playlists and channels: the probe lists them with `--flat-playlist --playlist-items 1-1000` (ADR-0016); the download runs
  yt-dlp with `--yes-playlist -i --no-quiet` and counts saved and skipped videos from its output, then checks and
  encodes each one with the preset.
- Each engine keeps an archive in `.scrappy/` (`gallery-dl.sqlite3`, `yt-dlp-archive.txt`), so a file saved before
  is skipped.
- Instagram, X and Facebook answer "not found" to a visitor who isn't signed in; Scrappy reads that as a sign-in
  wall and asks for one (ADR-0005).

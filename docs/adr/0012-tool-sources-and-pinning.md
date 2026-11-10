# ADR-0012: Tool sources and pinning

**Status:** Accepted; replaces the "Sources" bullet of ADR-0002 and says when ADR-0002's checksums are checked
**Date:** 2026-11-10

## Context

Building the tool manager (M2) found that ADR-0002's sources had moved on. gallery-dl's releases no longer attach
binaries; its official builds now come from `gdl-org/builds`, one release a day, macOS included. BtbN's FFmpeg builds
are daily too, and BtbN deletes them after about two weeks, keeping only each month's last build. A pin to a build
that gets deleted breaks the first run. ADR-0002 also didn't say when the pinned SHA-256 is checked, and yt-dlp
replaces its own file when it updates.

## Decision

- **Sources**, each pinned to one release in `scrappy-core::tools`' manifest with the SHA-256 GitHub publishes for
  the file:
  - yt-dlp: `yt-dlp/yt-dlp` releases (Windows, Linux, macOS).
  - Deno: `denoland/deno` releases (Windows, Linux, macOS on Apple silicon and Intel).
  - gallery-dl: `gdl-org/builds` releases (Windows, Linux, macOS).
  - FFmpeg and ffprobe: BtbN's month-end `autobuild-YYYY-MM-DD-*` releases, the latest stable branch, GPL
    (Windows, Linux). macOS has no download and uses an FFmpeg on `PATH` (Homebrew) until M14 decides.
- **Checksums are checked at download.** A download whose SHA-256 doesn't match is deleted and never unpacked or run.
  Installed tools aren't re-hashed before each run.
- **yt-dlp updates itself** to nightly in the background (ADR-0002). Its updater checks each update against yt-dlp's
  own published `SHA2-256SUMS`, so the updated file no longer matches Scrappy's pin, by design.
- **Updating a pin** is a manifest change: a new release, its URLs and its GitHub SHA-256s, in one commit.

## Options considered

- **BtbN's rolling `latest` release:** always there, but its files change daily, so no hash can be pinned.
- **gyan.dev FFmpeg builds:** versioned and kept, but Windows only, and a second source to trust.
- **Hashing every tool before each run:** catches tampering after install, but costs seconds on every start for a
  200 MB FFmpeg, and anything that can replace a file in the app data folder can already run code as the user.

## Consequences

- **Easier:** the first run downloads files that exist and match a known hash; gallery-dl works on macOS without CI.
- **Harder:** pins age. FFmpeg's stays valid for as long as BtbN keeps month-end builds, but yt-dlp's pin only
  matters for the first install, and the others need a bump now and then.
- **Later:** M14 can bundle or build macOS FFmpeg; a pin bump could become a script that reads GitHub's API.
- **Rules:** `.claude/rules/architecture/core.md`, `security.md`.

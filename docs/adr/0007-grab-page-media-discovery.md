# ADR-0007: Grab: finding everything downloadable on a page

**Status:** Accepted
**Date:** 2026-11-10

## Context

The Grab view takes a link and lists everything it can download: plain files, images, and videos that are only
streamed in chunks (HLS `.m3u8`, DASH `.mpd`), which must be fetched piece by piece and joined. The list needs filters
by type, sorting by size and clear colors per type. Tauri's own webview can intercept requests only on Windows, so it
can't do this on every platform.

## Decision

- **Two ways in, one list.** From the extension (ADR-0006), the page's real requests. Without it, a **deep scan**:
  core drives the user's installed Chrome, Brave or Edge headless over CDP (`chromiumoxide`) and records the network
  responses. No Chromium is bundled.
- **Classify** each find by MIME type and extension into video, audio, stream (HLS/DASH), image and other, with its
  size when known.
- **Streams** go to yt-dlp's generic extractor first (it handles HLS, DASH and AES-128) with the page's referer and
  headers; `vsd` is the fallback for what yt-dlp can't parse. DRM-protected streams are out of scope.
- Picked items run as normal jobs and land in `Grabs/<domain>/<date>/`.

## Options considered

- **Tauri's webview with request interception:** Windows-only (WebView2); WebKit on macOS and Linux can't.
- **HTML parsing only:** misses everything loaded by JavaScript, which is most streamed video.

## Consequences

- **Easier:** works on any site that plays in a normal browser, chunked streams included.
- **Harder:** the deep scan needs a Chromium browser installed, and some sites block headless browsers.
- **Later:** a new media type is one entry in the classifier.
- **Rules:** `.claude/rules/architecture/core.md`.

# ADR-0007: Grab: finding everything downloadable on a page

**Status:** Accepted; the deep scan runs as a job since ADR-0019, and follows linked pages since ADR-0029;
saving as found since ADR-0031; browser only, to a depth, since ADR-0032
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

## Implementation notes (M8)

- `grab/find.rs` sorts a request or link into video, audio, stream, image or other by its MIME type, else its
  extension; stream pieces (`.ts`, `.m4s`), page code and fonts are dropped. At most 2000 finds per page.
- The deep scan (`grab/scan.rs`) uses `chromiumoxide` without its browser downloader. `tools::browser()` finds Chrome,
  Brave or Edge. Each scan gets a new, empty profile in a temporary folder, so the user's cookies and history are never
  opened. The user agent drops "Headless". After the page loads, its videos are played muted and the scan waits 8
  seconds, then adds the page's image, media and anchor links.
- Picked files are fetched as they are (up to 50 GB) with the page as Referer; streams go through yt-dlp with the
  page as Referer and the default preset. Both are normal downloads and show in the Download list. The catalog kind is
  `video`, `audio`, `image` or `file`, and the site is the page's domain.
- `vsd` isn't added yet: it waits for a stream yt-dlp can't read.
- "Grab all" in the extension sends the bridge's `grab` request: the page, its title and everything the extension
  noted. The app sorts it, keeps it in `grab::Inbox` and brings its window forward; the window takes the page when it
  starts and whenever it gains focus, so no new event is needed (ADR-0010).

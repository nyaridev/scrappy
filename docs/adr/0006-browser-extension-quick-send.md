# ADR-0006: Browser extension: a quick-send popup over native messaging

**Status:** Accepted; who registers the host is superseded by
[ADR-0014](0014-builds-installers-and-host-registration.md) (the app does, from Settings).
**Date:** 2026-11-10

## Context

Nari wants a small extension for Chrome and Brave (and Firefox), not a second app: open it on a page, see the videos
and images that can be downloaded, click one, and it goes to Scrappy. The browser sees things the app can't: the
requests a page actually made (HLS/DASH chunks, media loaded by JavaScript) with their headers, and the user's session.

## Decision

- **WXT, one codebase** in `extension/`: `pnpm build` (Chrome, Brave, Edge) and `pnpm build:firefox`, both
  Manifest V3.
- **The popup only lists and sends.** It lists the tab's media from two sources: the media requests the page made
  (observed with `webRequest`, non-blocking) and a scan of the page (video, img, srcset, CSS backgrounds). Each entry
  shows type, size and quality, colored by type, with a Videos / Images filter. A click sends
  `{ url, kind, referer, headers, pageUrl, title }` to the app, which queues it. The extension does no downloading
  and keeps no settings, history or storage.
- **Native messaging, not a local server.** The browser starts `scrappy-host`, which relays length-prefixed JSON
  frames to the running app over a local socket (`interprocess`), and starts the app if it's closed. The browser only
  lets the extension IDs listed in the host manifest (`allowed_origins`, `allowed_extensions`) connect.
- **Cookies:** only when the app asks for one domain and the user confirms in the popup (ADR-0005).
- **The message format lives in `scrappy-core::bridge`,** and the app validates every message.
- ~~The installers register the host for Chrome, Brave, Edge and Firefox (M14)~~: the app does, from Settings
  (ADR-0014). In development a script does it too.

## Permissions

| Permission | Why |
|---|---|
| `nativeMessaging` | The only way to the app: `connectNative` starts scrappy-host |
| `webRequest` (non-blocking) | Sees the media a page loads by script (HLS and DASH playlists, files), with their size and the Referer, Origin and User-Agent they were requested with. Chrome needs `extraHeaders` to show Referer and Origin |
| `scripting` | Runs the page scan (videos, images, `srcset`, preview tags, CSS backgrounds) when the popup opens |
| `<all_urls>` (host) | `webRequest` and the scan work on any site. Firefox asks for it in the popup the first time |
| `cookies` | Reads one site's cookies, only after the app answered `needsCookies` for that site and the user picked "Use once" or "Remember" in the popup (ADR-0005). Only `lib/cookies.ts` calls it |

No `tabs` or `storage`. The tab's address and title come with the host permission. The requests the
background script saw are kept in memory only, per tab, and dropped when the tab navigates or closes; when the
browser stops an idle background script, they're lost, and the popup still has its page scan.

## Implementation notes (M5)

- The Chrome build carries a fixed public `key`, so the unpacked extension has the same ID on every machine
  (`agmacpepkdohlgbkbgnafcmjggjmeohj`), and the host manifest can name it. Firefox uses the ID `scrappy@nyaridev`.
  A store listing gets its own ID (M14), added to the host manifest beside this one.
- The message types are generated from `scrappy-core::bridge` into `extension/lib/types/` (`pnpm types`).
- The app's socket is named per user (`scrappy-<user>.sock`; a named pipe on Windows, `/tmp` on macOS, the abstract
  namespace on Linux). When the app can't be reached or started, scrappy-host answers with an error frame, so the
  popup shows why.
- A sent video uses the default preset; a sent page link goes to yt-dlp, so sites it knows work as in the window.
  Images are fetched straight into `Images/{site}/`, the site named after the page they were found on.
- `pnpm register-host` registers the debug host for Chrome, Brave, Edge, Chromium and Firefox in development.

## Options considered

- **A localhost HTTP or WebSocket server:** any local program could connect; it needs pairing tokens, a port and CORS
  handling.
- **The full app UI inside the extension:** two UIs to keep in step, and far more permissions.

## Consequences

- **Easier:** sending a video takes two clicks, and the app gets the real headers and referer protected streams need.
- **Harder:** a video send waits for yt-dlp to read the link (a few seconds) before the popup shows it as sent.
- **Harder:** the host must be registered per browser and per OS.
- **Later:** Safari would need a localhost fallback with pairing; Edge and Opera use the Chrome build.
- **Rules:** `.claude/rules/architecture/extension.md`, `security.md`.

## Implementation notes (M8)

- A third request, `grab`, carries the page's link and title and every request and link the popup noted (link, MIME
  type, size), at most 2000. The app checks it like a send, opens Grab with it and answers `opened` with the count.

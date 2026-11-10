# ADR-0006: Browser extension: a quick-send popup over native messaging

**Status:** Accepted
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
- The installers register the host for Chrome, Brave, Edge and Firefox (M14); in development a script does it.

## Options considered

- **A localhost HTTP or WebSocket server:** any local program could connect; it needs pairing tokens, a port and CORS
  handling.
- **The full app UI inside the extension:** two UIs to keep in step, and far more permissions.

## Consequences

- **Easier:** sending a video takes two clicks, and the app gets the real headers and referer protected streams need.
- **Harder:** the host must be registered per browser and per OS.
- **Later:** Safari would need a localhost fallback with pairing; Edge and Opera use the Chrome build.
- **Rules:** `.claude/rules/architecture/extension.md`, `security.md`.

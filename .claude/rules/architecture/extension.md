---
paths:
  - "extension/**"
  - "crates/scrappy-host/**"
---

# Browser extension and host

The extension is a quick-send popup: it shows the page as the app reads it (the video, when a site reader knows the
page), lists its videos and images, and sends the one the user picks to the app, or hands the whole page to the app's
Grab view ("Grab all"). A content script adds Download buttons to pages (ADR-0035). The app does all the work
(ADR-0006, ADR-0042).

- No downloading, settings, history or storage in the extension. If a feature needs one of those, it belongs in the app.
- One codebase for every browser: `pnpm build` (Chrome, Brave, Edge) and `pnpm build:firefox`, both Manifest V3.
  Browser differences go in `wxt.config.ts` or behind WXT's `import.meta.env.FIREFOX`, never in a second copy.
- Permissions are listed in `wxt.config.ts`, each one justified in ADR-0006. Add one only with that ADR updated.
- It talks to the app only through `sendToApp` in `extension/lib/app.ts` (`connectNative("com.nyaridev.scrappy")`).
  No fetch to localhost.
- Message types come from `scrappy-core::bridge` into `extension/lib/types/` (`pnpm types`); never edit them.
- Request headers leave the extension only from the allowlist (Referer, Origin, User-Agent); the app checks again.
- `lib/scan.ts` runs inside the page: it stays one self-contained function, with no imports at run time.
- In-page buttons (`entrypoints/buttons.content.ts`, `lib/buttons/`, `lib/sites/`) live in a closed shadow root and
  build their DOM with `createElement`, never `innerHTML` (YouTube enforces Trusted Types). They ask the background
  script (`lib/asks.ts`), which alone calls `sendToApp`. Cookies are never read for them: a sign-in goes through the
  popup.
- Cookies leave the extension only for one site, on the user's click in the popup: the site the app asked about
  (`needsCookies`, then "Use once" or "Remember"), or the tab's site through "Share sign-in" (`signIn`). Only
  `lib/cookies.ts` reads cookies.
- Whether a page is a video is the app's call (`look`, yt-dlp's site readers without its generic one), never a list
  of sites in the extension. The popup and the overlay's `blob:` videos ask it; a known page is sent as the page,
  not as a stream it loaded.
- Sizes in the popup are the response's, else the page's resource timing, else guessed from pixels and length
  (`sizeGuess` in `lib/media.ts`), shown with `~`.
- A button sends the start preset unless its place gives a `format` (label and an app preset's id, ADR-0043), as
  YouTube's MP4 and MP3 do. Those ids and `media/presets.rs` change together.
- The overlay also covers YouTube's embedded players (`lib/buttons/overlay.ts`); another site's embed is one more
  pattern there, sending that video's own page.

## scrappy-host

- It relays frames and nothing else: 4-byte length prefix plus JSON on stdin/stdout, to and from the app's local
  socket. It checks only the frame size (the app validates content).
- It starts the app when no app is listening: the app beside it, or the one named in an `app-path` file beside it
  (an AppImage's copy, ADR-0014).
- It ships inside each release build as a Tauri sidecar; the app connects the browsers to it from Settings
  (`bridge::connect`). The browser list and the extension IDs live in `bridge/register.rs` and in
  `scripts/register-host.mjs` (development): change both together.
- It stays small, with no dependency on `scrappy-core`. Its socket name and frame limit copy core's; change both
  together.
- When the app can't be reached it answers with an error frame, never silence.

## Getting the extension

- Settings → Browser downloads the Chromium zip for the app's own version from its GitHub release and unpacks it into
  `browser-extension/` in the app's data folder, for Load unpacked (`bridge/extension.rs`, ADR-0040). The release
  workflow's zip names (`scrappy-extension-<version>-chrome.zip`) and `bridge/extension.rs` change together.

## How to add a site's buttons

1. Look at the site's markup in a browser and pick hooks that outlast its class names: roles, `aria-label`s, ids,
   link shapes (`/status/<id>`).
2. Add `lib/sites/<site>.ts` exporting a `Site` (its hosts, look, dark mode and `places()`), and list it in
   `lib/sites/index.ts`. A new look goes in `lib/buttons/looks.ts`, measured from the site's own buttons.
3. Send the item's own link, without playlist or tracking parameters.

## How to add something to the popup

1. Put the UI in `extension/entrypoints/popup/`, and code shared with the background script in `extension/lib/`.
2. If it needs the app, add the message to `scrappy-core::bridge` (with a test), then send it from the popup.

---
paths:
  - "extension/**"
  - "crates/scrappy-host/**"
---

# Browser extension and host

The extension is a quick-send popup: it lists the current page's videos and images and sends the one the user picks
to the app, or hands the whole page to the app's Grab view ("Grab all"). The app does all the work (ADR-0006).

- No downloading, settings, history or storage in the extension. If a feature needs one of those, it belongs in the app.
- One codebase for every browser: `pnpm build` (Chrome, Brave, Edge) and `pnpm build:firefox`, both Manifest V3.
  Browser differences go in `wxt.config.ts` or behind WXT's `import.meta.env.FIREFOX`, never in a second copy.
- Permissions are listed in `wxt.config.ts`, each one justified in ADR-0006. Add one only with that ADR updated.
- It talks to the app only through `sendToApp` in `extension/lib/app.ts` (`connectNative("com.nyaridev.scrappy")`).
  No fetch to localhost.
- Message types come from `scrappy-core::bridge` into `extension/lib/types/` (`pnpm types`); never edit them.
- Request headers leave the extension only from the allowlist (Referer, Origin, User-Agent); the app checks again.
- `lib/scan.ts` runs inside the page: it stays one self-contained function, with no imports at run time.
- Cookies leave the extension only for the one site the app asked about (`needsCookies`), after the user picks
  "Use once" or "Remember" in the popup. Only `lib/cookies.ts` reads cookies.

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

## How to add something to the popup

1. Put the UI in `extension/entrypoints/popup/`, and code shared with the background script in `extension/lib/`.
2. If it needs the app, add the message to `scrappy-core::bridge` (with a test), then send it from the popup.

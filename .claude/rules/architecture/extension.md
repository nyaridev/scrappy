---
paths:
  - "extension/**"
  - "crates/scrappy-host/**"
---

# Browser extension and host

The extension is a quick-send popup: it lists the current page's videos and images and sends the one the user picks
to the app. The app does all the work (ADR-0006).

- No downloading, settings, history or storage in the extension. If a feature needs one of those, it belongs in the app.
- One codebase for every browser: `pnpm build` (Chrome, Brave, Edge) and `pnpm build:firefox`, both Manifest V3.
  Browser differences go in `wxt.config.ts` or behind WXT's `import.meta.env.FIREFOX`, never in a second copy.
- Permissions are listed in `wxt.config.ts`, each one justified in ADR-0006. Add one only with that ADR updated.
- It talks to the app only through `browser.runtime.connectNative("com.nyaridev.scrappy")`. No fetch to localhost.
- Messages follow the format in `scrappy-core::bridge`. When the format changes, change both sides in the same commit.
- Cookies leave the extension only for the one domain the app asked about, after the user confirms in the popup.

## scrappy-host

- It relays frames and nothing else: 4-byte length prefix plus JSON on stdin/stdout, to and from the app's local
  socket. It checks only the frame size (the app validates content).
- It starts the app when no app is listening.
- It stays small, with no dependency on `scrappy-core`.

## How to add something to the popup

1. Put the UI in `extension/entrypoints/popup/`, and code shared with the background script in `extension/lib/`.
2. If it needs the app, add the message to `scrappy-core::bridge` (with a test), then send it from the popup.

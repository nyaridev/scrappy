# ADR-0040: The extension's files from the release

**Status:** Accepted
**Date:** 2026-11-13
**Amends:** ADR-0014 (the app fetches the extension for its own version)

## Context

The extension isn't in any browser's store; it's loaded unpacked. Getting it means finding the release, downloading the
right zip, unpacking it, and keeping it where the browser can load it from. The app knows its own version, and every
release carries the extension built with it (ADR-0014).

## Decision

- **Settings → Browser has "Download extension":** the app reads the release for its own version
  (`v<version>`) from GitHub's API, downloads `scrappy-extension-<version>-chrome.zip`, checks it against the SHA-256
  GitHub gives for the asset, and unpacks it into `browser-extension/` in the app's data folder. Then it opens that
  folder, for the user to load in the browser's extensions page (Developer mode, Load unpacked).
- **Once it's there,** the button reads "Open extension folder" and only opens it. A newer Scrappy finds an older copy
  (the manifest's version isn't its own) and offers the download again; the new files replace the old ones in the
  same folder, so the browser's Reload picks them up.
- **Chromium only** (Chrome, Brave, Edge): Firefox doesn't keep an unpacked extension after a restart; its zip stays in
  the release for a temporary load.
- `bridge` owns it (`bridge/extension.rs`), beside connecting the browsers. Requests go through the proxy
  (ADR-0034); the zip is unpacked with each entry kept inside the folder.

## Options considered

### A: Link to the release page
- **Pros:** no code.
- **Cons:** the user finds the asset, unpacks it and keeps the folder somewhere stable themselves.

### B: Ship the extension inside the installer
- **Pros:** no download.
- **Cons:** every platform's bundle grows and carries a copy most never use; the release already has the zip.

### C: Fetch the release's zip for this version (chosen)
- **Pros:** always the extension that matches the app; one stable folder to load and reload.
- **Cons:** needs a published release for the version: a development build has none.

## Consequences

- **Easier:** getting the extension is one click and one "Load unpacked".
- **Harder:** a version without a published release can't fetch its extension.
- **Rules:** `extension.md`, `security.md` (the zip is checked against GitHub's digest).

# ADR-0037: Updates from GitHub releases, installers only

**Status:** Accepted
**Date:** 2026-11-13
**Amends:** ADR-0014 (no portable Windows build; releases carry signed update files)

## Context

A new version means finding the release, downloading the installer and running it. Nari's ume-sync app updates
itself: it reads the repository's tags with git, downloads the installer with the GitHub CLI and runs it, because its
repository is private and it has no HTTP client. Scrappy's repository is public and the app already has one, so it
can do the same more simply, and check what it installs. The portable Windows zip can't update itself and isn't
wanted.

## Decision

- **Tauri's updater plugin** reads `latest.json` from the latest GitHub release
  (`https://github.com/nyaridev/scrappy/releases/latest/download/latest.json`), downloads the installer it names, checks
  its signature against the public key in `src-tauri/tauri.release.conf.json`, and runs it. On Windows that's the NSIS
  installer in passive mode; it restarts Scrappy.
- **The signing key** is made once (`pnpm tauri signer generate`, without a password); its private half is a GitHub
  secret (`TAURI_SIGNING_PRIVATE_KEY`) and a file kept outside the repository, never in it. Only the release
  workflow signs: it adds `src-tauri/tauri.updater.conf.json` (`createUpdaterArtifacts`), so a local `build.bat` needs
  no key, and `tauri-action` attaches `latest.json` and the signatures to the draft release.
- **When it checks:** at start and every 6 hours, while "Check for updates" is on in Settings (on by default, saved
  in the preferences); and when the user clicks Check now. Requests go through the proxy (ADR-0034).
- **What the user sees:** an update icon in the header when a newer version is out, which opens Settings; Settings'
  Scrappy section shows the version, what was found and "Update and restart". Installing happens only on that click.
- **Where it can't:** a debug build doesn't check (it would replace itself with a release). A failed check is one
  quiet line in Settings, never a popup.
- **Installers only:** `build.bat` and the release workflow build the installers (NSIS, the macOS disk image, the
  AppImage and `.deb`) and the extension's zips. The portable Windows exe and zip are gone.
- The shell owns it (`src-tauri/src/update.rs`): it's Tauri's plugin, and core has no Tauri code. Core holds the
  status the window sees (`app::UpdateStatus`); each change reaches the window as a new event, `update-changed`.
- `requireSignedVersion` is on: an update's signature must carry the version `latest.json` names, so an old signed
  installer can't be passed off as a new one.

## Options considered

### A: ume-sync's way: git tags, gh, the installer
- **Pros:** no signing key; proven in ume-sync.
- **Cons:** needs git and a logged-in GitHub CLI on the user's machine; nothing checks the installer it runs.

### B: The GitHub API and a plain download of the installer
- **Pros:** no plugin, no key.
- **Cons:** the installer is unchecked; the app would write what the plugin already does (download, run, restart).

### C: Tauri's updater plugin (chosen)
- **Pros:** signed updates, checked before they run; one plugin does the check, download and install; works for
  every platform's build.
- **Cons:** a signing key to keep; a lost key means users update by hand once.

## Consequences

- Releases must be published (not left as drafts) for `latest/download` to find them.
- Versions before the first with this are updated by hand once.
- The plugin talks to the network from the shell; it sends nothing about the user (ADR-0005).
- **Rules:** `shell.md` (`update.rs`), `security.md` (updates are signed).

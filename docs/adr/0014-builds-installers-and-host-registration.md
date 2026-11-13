# ADR-0014: Builds, installers and connecting the browsers

**Status:** Accepted; the portable Windows build is gone and releases carry signed update files:
[ADR-0037](0037-updates-from-github-releases.md); the app fetches the extension for its version from the release:
[ADR-0040](0040-extension-files-from-the-release.md)
**Date:** 2026-11-10
**Supersedes:** ADR-0006's "the installers register the host"

## Context

Scrappy is built by hand on Windows today (`build.bat`, a portable exe). It needs checks on every change and
installers for Windows, macOS and Linux. The extension's host (`scrappy-host`) must sit beside the app, which it
starts when it isn't running, and each browser must be told where the host is. ADR-0006 planned to have the installers
do that, but a macOS disk image and a Linux AppImage have no install step to run it, and an installer can't see
browsers installed later.

## Decision

- **Checks:** a GitHub Actions workflow runs `pnpm check` on Windows, macOS and Linux on every push to `main` and
  every pull request.
- **Releases:** pushing a `v*` tag runs a workflow that builds with `tauri-action` into a draft GitHub release: a
  Windows NSIS installer (per user, no admin) and a portable zip (`Scrappy.exe` with `scrappy-host.exe`), a macOS
  disk image (one universal app for Apple Silicon and Intel, unsigned for now), and a Linux AppImage and `.deb`. The
  extension's Chrome and Firefox zips are attached to the same release.
- **The host ships inside each build** as a Tauri sidecar (`bundle.externalBin`), so it lands beside the app. Only
  release builds bundle it: `src-tauri/tauri.release.conf.json` turns bundling on, and its build step builds the
  host for the target (`scripts/stage-host.mjs`). Development and `pnpm check` don't need it.
- **The app connects the browsers,** from Settings → Browser extension, for the app that is running: on Windows a
  host manifest in the app's data folder and a per-user registry key for Chrome, Brave, Edge, Chromium and Firefox;
  on macOS and Linux a manifest in each installed browser's own folder. "Disconnect" removes them. An AppImage's
  files vanish when it closes, so its host is copied into the app's data folder first, with the AppImage's path
  beside it for the host to start. This lives in `bridge` (ADR-0006's module for the extension).
- `pnpm register-host` stays for development without the app running.

## Options considered

- **Installer scripts** (NSIS hooks, `.deb` post-install): no click needed, but nothing for the disk image and the
  AppImage, and browsers installed later stay unconnected.
- **Bundling the host in every build, development too:** simpler config, but `pnpm check` and `start-dev.bat` would
  first need a host build for the exact target.

## Consequences

- **Easier:** one place connects every browser on every system, and it can say which ones are connected.
- **Harder:** the user clicks "Connect browsers" once after installing; a moved portable app needs it again.
- **Later:** signing and notarizing for macOS and Windows; an updater; a store listing's extension ID.
- **Rules:** `.claude/rules/architecture/extension.md`, `.claude/rules/architecture/shell.md`.

## Implementation notes (M14)

- `.github/workflows/check.yml` builds the window first (`pnpm build`): the shell's compile needs `dist/`.
- `.github/workflows/release.yml` passes `--config src-tauri/tauri.release.conf.json` to `tauri-action`, one runner
  per system: `--bundles nsis` (Windows), `--target universal-apple-darwin --bundles app,dmg` (macOS),
  `--bundles appimage,deb` (Linux, Ubuntu 22.04 for an older glibc). The Windows runner also zips `Scrappy.exe` with
  the staged host as the portable build. A second job zips the extension (`wxt zip`) and uploads both zips.
- `scripts/stage-host.mjs` reads `TAURI_ENV_TARGET_TRIPLE` (the machine's own target without it), builds the host
  with `--target`, and copies it to `src-tauri/binaries/` (ignored by git); for a universal build it builds both
  Mac architectures and joins them with `lipo`. Tried locally with
  `pnpm tauri build --debug --no-bundle --config src-tauri/tauri.release.conf.json`: the host lands beside the app.
- `bridge/register.rs` holds the browser list and extension IDs, as `scripts/register-host.mjs` does; Windows writes
  through `winreg` (HKCU only, no admin). Connected means the browser's manifest names this app's host, so a moved
  portable app shows as not connected until it's connected again.
- `build.bat` makes the same Windows release locally (installer, portable app and zip, extension zips) into
  `release\`; a Windows PC can't build the macOS and Linux ones, which come from the workflow.

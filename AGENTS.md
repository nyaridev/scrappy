# AGENTS.md — Scrappy

Guide for AI agents (Claude Code, Cursor, Codex…) working in this repo. In Claude Code, the workspace and category
rules load from the `CLAUDE.md` files above this folder; this file covers only what's specific here.

## What this is

A desktop app (Windows, macOS, Linux) for Nari. It downloads videos and media from YouTube, X, Instagram, Facebook and
any other site, finds everything downloadable on a page (including chunked streams), crawls and mirrors sites, hunts
for images by topic with duplicate removal, and keeps a gallery and stats of it all. A small browser extension sends a
page's videos and images to the app. Stage: M15 (changing the library folder) and M16–M17 are done; `docs/adr/README.md` tracks the milestones.

## Commands

| Command | What |
|---|---|
| `start-dev.bat` | Opens the app with hot reload (`pnpm tauri dev`), installing packages on the first run |
| `build.bat` | Builds for Windows into `release\`: the installer and the extension's Chrome and Firefox zips |
| `git tag vX.Y.Z` + push the tag | Builds the signed installers and `latest.json` on GitHub into a draft release (`.github/workflows/release.yml`); publish it for the app's updater to see it (ADR-0037) |
| `pnpm dev` | The window alone in a browser, on fake data (port 4430) |
| `pnpm check` | Size and boundary checks, tsc (app and extension), Biome, Vitest, `cargo test`, clippy. Must pass before a task is done |
| `cargo test -p scrappy-core -- --ignored` | Downloads every pinned tool from GitHub (about 300 MB) and searches the three image engines: run it after changing a pin, or when a Hunt engine source fails |
| `pnpm types` | Regenerates `src/lib/types/` and `extension/lib/types/` from the Rust types (ts-rs) |
| `pnpm format` | Biome format and fix |
| `pnpm -C extension build` / `build:firefox` | Builds the extension into `extension/.output/` (`zip` / `zip:firefox` for the zips) |
| `pnpm register-host` | Registers the debug scrappy-host with Chrome, Brave, Edge, Chromium and Firefox (`--remove` undoes it); the app does it from Settings → Browser extension |

Rust builds put their intermediate files in the workspace's `_cache/`; the final binaries land in `target/`.

## Layout

| Folder | What |
|---|---|
| `crates/scrappy-core/` | All logic, one module per system. Never uses Tauri |
| `crates/scrappy-host/` | The native-messaging relay the browser starts |
| `src-tauri/` | The Tauri shell: setup, `window.rs` (the frameless window), `events.rs` (job events), `commands/<area>.rs`, capabilities |
| `src/` | The window: `app/` (header and view list), `features/<view>/`, `lib/backend.ts` (with its preview data in `lib/fakeData.ts`), `lib/types/` (generated) |
| `extension/` | The WXT browser extension: a background script, one popup, and the in-page buttons (`entrypoints/buttons.content.ts`, `lib/buttons/`, one file per site in `lib/sites/`); `lib/types/` is generated |
| `docs/adr/` | Architecture overview, decisions and milestone tracker |
| `scripts/` | `check-size.mjs`, `check-boundaries.mjs`, `register-host.mjs` |
| `assets/brand/` | The README's banners, dark and light, and `icon.svg`, the app icon (`pnpm tauri icon assets/brand/icon.svg` makes `src-tauri/icons/`) |

## Architecture

All logic lives in the Rust core crate; Tauri is a thin shell around it, and the React + iro-ui window only shows what
Rust sends. Read `docs/adr/README.md` first.

| Rules | Applies to |
|---|---|
| `.claude/rules/architecture/overview.md` | everything |
| `.claude/rules/architecture/code-organization.md` | `crates/**`, `src-tauri/src/**`, `src/**`, `extension/entrypoints/**`, `extension/lib/**`, `scripts/**` |
| `.claude/rules/architecture/core.md` | `crates/scrappy-core/**`, `packs/**` |
| `.claude/rules/architecture/shell.md` | `src-tauri/**` |
| `.claude/rules/architecture/frontend.md` | `src/**`, `index.html`, `vite.config.ts` |
| `.claude/rules/architecture/extension.md` | `extension/**`, `crates/scrappy-host/**` |
| `.claude/rules/architecture/security.md` | `crates/**`, `src-tauri/**`, `extension/**` |

## Rules

- UI text is short and plain, with no flavor text: `.claude/rules/ui-text.md`.
- iro-ui comes from a release tag (`github:nyaridev/iro-ui#vX.Y.Z`), never a path or a branch.
- `src/lib/types/` and `extension/lib/types/` are generated: change the Rust type and run `pnpm types`.
- The window is frameless (ADR-0011). If the custom title bar misbehaves on a machine, turn on Settings → "Use the
  system title bar".
- The external tools (yt-dlp, Deno, gallery-dl, ffmpeg) are never committed; `scrappy-core::tools` downloads them.
- Never commit cookies, cookies.txt files, logs or a test library folder.

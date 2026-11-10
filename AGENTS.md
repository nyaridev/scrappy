# AGENTS.md — Scrappy

Guide for AI agents (Claude Code, Cursor, Codex…) working in this repo. In Claude Code, the workspace and category
rules load from the `CLAUDE.md` files above this folder; this file covers only what's specific here.

## What this is

A desktop app (Windows, macOS, Linux) for Nari. It downloads videos and media from YouTube, X, Instagram, Facebook and
any other site, finds everything downloadable on a page (including chunked streams), crawls and mirrors sites, hunts
for images by topic with duplicate removal, and keeps a gallery and stats of it all. A small browser extension sends a
page's videos and images to the app. Stage: M0 (scaffold) is done; `docs/adr/README.md` tracks the milestones.

## Commands

| Command | What |
|---|---|
| `start-dev.bat` | Opens the app with hot reload (`pnpm tauri dev`), installing packages on the first run |
| `build.bat` | Builds the portable `release\Scrappy.exe` |
| `pnpm dev` | The window alone in a browser, on fake data (port 4430) |
| `pnpm check` | Size and boundary checks, tsc (app and extension), Biome, Vitest, `cargo test`, clippy. Must pass before a task is done |
| `pnpm types` | Regenerates `src/lib/types/` from the Rust types (ts-rs) |
| `pnpm format` | Biome format and fix |
| `pnpm -C extension build` / `build:firefox` | Builds the extension into `extension/.output/` |

Rust builds put their intermediate files in the workspace's `_cache/`; the final binaries land in `target/`.

## Layout

| Folder | What |
|---|---|
| `crates/scrappy-core/` | All logic, one module per system. Never uses Tauri |
| `crates/scrappy-host/` | The native-messaging relay the browser starts |
| `src-tauri/` | The Tauri shell: setup, `commands/<area>.rs`, capabilities |
| `src/` | The window: `app/` (header and view list), `features/<view>/`, `lib/backend.ts`, `lib/types/` (generated) |
| `extension/` | The WXT browser extension (one popup) |
| `docs/adr/` | Architecture overview, decisions and milestone tracker |
| `scripts/` | `check-size.mjs`, `check-boundaries.mjs` |

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

- iro-ui comes from a release tag (`github:nyaridev/iro-ui#vX.Y.Z`), never a path or a branch.
- `src/lib/types/` is generated: change the Rust type and run `pnpm types`.
- The external tools (yt-dlp, Deno, gallery-dl, ffmpeg) are never committed; `scrappy-core::tools` downloads them.
- Never commit cookies, cookies.txt files, logs or a test library folder.

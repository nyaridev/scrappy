# ADR-0001: Rust core, thin Tauri shell, display-only UI

**Status:** Accepted
**Date:** 2026-11-10

## Context

Scrappy downloads, encodes, crawls, sniffs pages and handles cookies. That work touches the disk, the network and
secrets, so it has to live in one place that can be tested and trusted. It ships on Windows, macOS and Linux, and has
a browser extension and a native-messaging host that need some of the same code. Nari wants the window to only show
state and pass clicks on, with no logic in TypeScript.

## Decision

A Cargo workspace with three crates, plus the window and the extension:

- `crates/scrappy-core/`: all logic, one module per system (`library`, `jobs`, `tools`, `media`, `grab`, `crawl`,
  `hunt`, `dedupe`, `vault`, `bridge`, `settings`, `logging`). It never depends on Tauri.
- `src-tauri/`: the shell. It opens the window, registers commands (`commands/<area>.rs`, one core call each) and
  forwards progress as events. It holds no logic.
- `crates/scrappy-host/`: the native-messaging relay (ADR-0006). It doesn't depend on core.
- `src/`: React 19 + iro-ui. It shows what Rust sends and calls Rust only through `src/lib/backend.ts`.
- `extension/`: the WXT browser extension (ADR-0006).

The window is a header with the view tabs centered in it (Home, Download, Grab, Scrape, Gallery) and Settings as a
button at its right. Each view lives in `src/features/<view>/`. iro-ui is installed from a release tag
(`github:nyaridev/iro-ui#vX.Y.Z`), and the app's CSS uses its tokens only.

## Options considered

- **One `src-tauri` crate, like Workstation:** simpler at first, but the host and a later CLI couldn't reuse the logic,
  and nothing would stop logic from leaking into commands.
- **Logic in TypeScript where it's easier (filtering, parsing):** rejected. It splits behavior across two languages
  and gives the webview more reach than it needs.
- **A sidebar for the views:** tried in M0; Nari chose tabs centered in a header.

## Consequences

- **Easier:** core is testable with `cargo test` and no window, and Cargo itself enforces the crate boundary.
- **Harder:** every feature needs a command, a type and a `backend.ts` function, even small ones.
- **Later:** a CLI is one more crate on top of core. A new view is a folder in `src/features/` plus a line in
  `src/app/views.ts`. A new system is a module in core.
- **Rules:** `.claude/rules/architecture/overview.md`, `core.md`, `shell.md`, `frontend.md`.

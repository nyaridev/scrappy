# ADR-0010: Typed commands, jobs and events

**Status:** Accepted
**Date:** 2026-11-10

## Context

Scrappy will have dozens of commands and long-running work (downloads, encodes, crawls) that reports progress and can
be cancelled. Hand-written TypeScript copies of Rust types drift. The window must also be previewable in a plain
browser (`pnpm dev`), as in Workstation and PrepKit.

## Decision

- **Types:** structs the UI sees derive `ts_rs::TS` with `#[ts(export)]` and `#[serde(rename_all = "camelCase")]`.
  `cargo test` (or `pnpm types`) writes them to `src/lib/types/`, which is committed and never edited by hand.
- **One door:** every `invoke` and `listen` is in `src/lib/backend.ts`, which serves fake data outside Tauri.
- **Jobs:** long work is a job in `scrappy-core::jobs`, with an id, a kind, a cancel flag and a concurrency limit per
  kind. Commands start a job and return its id at once.
- **Events:** core reports progress through a callback, and the shell forwards it as one `job-progress` event
  (`{ id, phase, done, total, speed, eta, message }`) and a `job-finished` event (`{ id, result }`). Views filter by
  id.

## Options considered

- **`tauri-specta`:** generates the commands too, but has stayed a release candidate; ts-rs is stable and only does
  types.
- **Hand-written types, as in Workstation:** fine for a handful of commands, but drifts at this size.

## Consequences

- **Easier:** a Rust type change breaks the TypeScript build instead of the running app.
- **Harder:** a new type needs `pnpm types` before the UI sees it.
- **Later:** a CLI can drive the same jobs and print the same progress.
- **Rules:** `.claude/rules/architecture/shell.md`, `frontend.md`.

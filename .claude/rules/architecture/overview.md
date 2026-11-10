# Architecture overview

All logic is in the Rust core crate. Tauri is a thin shell around it, and the React + iro-ui window only shows what
Rust sends. Read `docs/adr/README.md` first.

## Modules

| Module | Owns | May depend on |
|---|---|---|
| `crates/scrappy-core/` | every behavior | crates.io crates; never `tauri` |
| `src-tauri/` | the window, commands, capabilities, events | `scrappy-core`, `tauri` |
| `crates/scrappy-host/` | relaying extension frames to the app | std, a local-socket crate; not `scrappy-core` |
| `src/` | showing state, passing clicks on | `iro-ui`, React; Rust only through `src/lib/backend.ts` |
| `extension/` | listing a page's media, sending a pick | WXT, React; the app only through native messaging |

## Where new things go

| New thing | Goes in | Recipe |
|---|---|---|
| Logic of any kind (parsing, files, network, processes) | a module in `crates/scrappy-core/src/` | `core.md` |
| A site, preset or source pack | data in core, or `packs/` | `core.md` |
| Something the window calls | `src-tauri/src/commands/<area>.rs` | `shell.md` |
| A type the window sees | the core module that owns it, with `#[ts(export)]` | `shell.md` |
| A view (tab) | `src/features/<view>/`, listed in `src/app/views.ts` | `frontend.md` |
| A component one view uses | that view's folder; several views: `src/components/` | `frontend.md` |
| Anything in the browser popup | `extension/entrypoints/` or `extension/lib/` | `extension.md` |
| Cookies, secrets, user input checks, logging | `vault`, `bridge`, `logging` in core | `security.md` |

## Never

- Logic, file access or network calls in `src/` or `extension/`. They show and send; Rust decides and does.
- `@tauri-apps/*` imports outside `src/lib/backend.ts` (`scripts/check-boundaries.mjs` fails on it).
- `use tauri` in `scrappy-core` (Cargo refuses it: core has no Tauri dependency).

## Changing the architecture

A change to modules, dependencies, layout or any ADR's decision needs a new ADR in `docs/adr/` (the old one is marked
superseded, never deleted), an update to `docs/adr/README.md` and to these rules, before the code changes.

# Architecture

Scrappy is a desktop app for downloading videos and media from any site, grabbing what a page streams, crawling and
scraping images, and browsing it all. All logic lives in the Rust core crate; Tauri is a thin shell around it, and the
React + iro-ui window only shows what Rust sends.

## Shape

```
crates/scrappy-core/   all logic, one module per system; never uses Tauri
crates/scrappy-host/   native-messaging relay the browser starts (M5)
src-tauri/             Tauri 2 shell: lib.rs (setup), commands/<area>.rs, capabilities/
src/                   React 19 + iro-ui: app/ (header, views), features/<view>/, lib/backend.ts, lib/types/ (generated)
extension/             WXT browser extension: one popup, Chrome/Brave/Edge and Firefox builds
packs/                 built-in source packs for Hunt, TOML (M13)
docs/adr/              this overview and the decisions
scripts/               check-size.mjs, check-boundaries.mjs
```

## Modules

| Module | Owns | May depend on |
|---|---|---|
| `scrappy-core` | every behavior: library, jobs, tools, media, grab, crawl, hunt, dedupe, vault, bridge, settings, logging | crates.io crates; never `tauri` |
| `src-tauri` | the window, commands, capabilities, forwarding progress as events | `scrappy-core`, `tauri` |
| `scrappy-host` | relaying extension frames to the app | std, a local-socket crate; not `scrappy-core` |
| `src/` | showing state, passing clicks on | `iro-ui`, React; Rust only through `src/lib/backend.ts` |
| `extension/` | listing a page's media, sending a pick | WXT, React; the app only through native messaging |

Inside core, modules use each other only through their `pub` API. `jobs`, `logging`, `settings` and `library` are
the base; `tools` → `media` → `grab`/`crawl`/`hunt` build on them; `vault` and `bridge` are used by the rest.

## Decisions

| ADR | Title | Status |
|---|---|---|
| [0001](0001-rust-core-thin-shell-display-only-ui.md) | Rust core, thin Tauri shell, display-only UI | Accepted |
| [0002](0002-external-engines-as-managed-tools.md) | External engines as managed tools | Accepted |
| [0003](0003-editor-safe-encoding-presets.md) | Editor-safe encoding presets | Accepted |
| [0004](0004-library-folder-and-catalog.md) | The library folder and its catalog | Accepted |
| [0005](0005-security-and-secrets.md) | Security and secrets | Accepted |
| [0006](0006-browser-extension-quick-send.md) | Browser extension: a quick-send popup over native messaging | Accepted |
| [0007](0007-grab-page-media-discovery.md) | Grab: finding everything downloadable on a page | Accepted |
| [0008](0008-crawl-and-offline-mirror.md) | Crawl and offline mirror | Accepted |
| [0009](0009-hunt-source-packs-and-dedupe.md) | Hunt: source packs, query detection and duplicate removal | Accepted |
| [0010](0010-typed-commands-jobs-and-events.md) | Typed commands, jobs and events | Accepted |

## Milestones

| Milestone | Goal | Status |
|---|---|---|
| M0 | Scaffold | done |
| M1 | Library and first-run setup | planned |
| M2 | Tool manager (yt-dlp, Deno, gallery-dl, ffmpeg) | planned |
| M3 | Download view: YouTube formats, presets, queue | planned |
| M4 | Home: stats and recent downloads | planned |
| M5 | Extension popup and host bridge | planned |
| M6 | Cookie vault | planned |
| M7 | Social and bulk downloads (X, Instagram, Facebook) | planned |
| M8 | Grab view | planned |
| M9 | Gallery | planned |
| M10 | Duplicate removal | planned |
| M11 | Image crawl with limits | planned |
| M12 | Site mirror | planned |
| M13 | Hunt with source packs | planned |
| M14 | Windows, macOS and Linux builds in CI, installers | planned |

### M1: Library and first-run setup

- [ ] 1. `settings`: load and save the app settings (the library root first) as JSON in the app config folder.
- [ ] 2. `library`: check a folder (exists, writable, empty or not) and create the layout from ADR-0004.
- [ ] 3. `library`: open or create `<root>/.scrappy/library.db` (`rusqlite`, bundled) with numbered migrations and the
  downloads table.
- [ ] 4. `library`: path templates and name sanitizing, with tests (carried over from `video-downloader`).
- [ ] 5. Shell: `library_status`, `check_folder` and `setup_library` commands; the folder picker opens from Rust
  (`tauri-plugin-dialog`'s Rust API), so the window gets no dialog permission.
- [ ] 6. UI: the first-run screen before the tabs (pick a folder, "this folder isn't empty, use it anyway?", create),
  with `?setup` and `?notEmpty` previews in `backend.ts`.
- [ ] 7. Done checks.

### Later

- Firefox's MV3 build needs `browser_specific_settings.gecko.id` before it can be signed (M5).
- Pick an app icon; `src-tauri/icons/` still holds the generator's.
- Decide Scrappy's own licence before the repo goes public (the external tools are GPL; ADR-0002).

## Rules

The rules agents follow are in `.claude/rules/architecture/`:

- `overview.md`: the modules, where new things go, and how architecture changes are made.
- `code-organization.md`: size limits, section order and dividers.
- `core.md`: scrappy-core's modules and how to add a system, a site, a preset or a source pack.
- `shell.md`: thin commands, capabilities and events.
- `frontend.md`: the display-only window and how to add a view.
- `extension.md`: the popup and the host.
- `security.md`: cookies, secrets, inputs and logs.

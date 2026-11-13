---
paths:
  - "src-tauri/**"
---

# Tauri shell

`src-tauri/` opens the window, registers commands and forwards progress. It holds no logic.

- `lib.rs` wires things up: setup (logging, state, the extension's listener, opening the window), plugins,
  `generate_handler!`. Nothing else. The single-instance plugin comes first, so a second start only brings the
  window forward.
- `window.rs` builds the main window with the frame `scrappy_core::app::window_frame()` picks (ADR-0011). Windows
  come from Rust, not `tauri.conf.json`.
- `events.rs` turns core's job reports into the `job-progress` and `job-finished` events, and the library watcher's
  calls into `library-changed`.
- `autostart.rs` turns starting with the system on or off from the preference (ADR-0036); `update.rs` checks for,
  downloads and installs Scrappy's own updates with Tauri's updater plugin and reports each change as
  `update-changed` (ADR-0037). Both leave the system alone in debug builds. `tray.rs` is the tray icon and its menu,
  and hides the window on close while "Close to tray" is on (ADR-0038).
- `protocol.rs` answers the `scrappy:` app protocol the Gallery's player streams from (ADR-0023): core's
  `library::serve` checks and reads, the shell only builds the HTTP response. No other protocol without an ADR.
- A plain `#[tauri::command] pub fn` runs on the window's thread and freezes it while it works. Any command that
  touches files, the catalog, the network or another process is `#[tauri::command(async)]` (or `async fn`); only
  instant ones (window controls, reading a value in memory) stay plain. A command that starts a job is `async`:
  `Jobs::start` needs the Tokio runtime.
- Commands live in `src/commands/<area>.rs`, one file per view or system, listed in `commands/mod.rs`.
- A command parses its arguments, makes one `scrappy_core` call and returns the result. Any `if` about behavior
  belongs in core.
- Long work: the command starts a core job and returns its id. Progress is forwarded as `job-progress`, the result as
  `job-finished` (ADR-0010). The library watcher sends `library-changed` (ADR-0015), the updater
  `update-changed` (ADR-0037). No other event names without an ADR change.
- Return types come from core with `ts_rs` exports; the shell defines no types the window sees.
- Errors go back as `Result<T, String>` with a readable message.

## Capabilities and config

- `capabilities/default.json` grants `core:default` and the window-control permissions from ADR-0011, nothing else.
  Adding any plugin permission (fs, shell, http, dialog, opener…) to the window needs an ADR-0005 change. Plugins Rust
  calls itself (a folder picker opened from Rust) need no window permission.
- Keep the CSP in `tauri.conf.json` strict. Loosen one source at a time, and only for something the window loads.
  Library pictures reach the window as `data:` URLs (ADR-0004, M9), so they need no asset protocol; videos and audio
  stream through `scrappy:`, the one `media-src` (ADR-0023).
- Release builds merge `tauri.release.conf.json`: bundling on, the `scrappy-host` sidecar staged by
  `scripts/stage-host.mjs` (ADR-0014), and the Windows installer's hooks, `windows/hooks.nsh`: the "Start Scrappy
  with Windows" box, handed to the app through the registry, which `autostart::restore` takes once (ADR-0041). Keep `tauri.conf.json` free of the sidecar, so development needs no host build.
- Dev server port: 4430 (`vite.config.ts`, `tauri.conf.json`, `.claude/launch.json`, `start-dev.bat`).

## How to add a command

1. Write the logic in core first, with its tests.
2. Add `#[tauri::command(async)] pub fn <name>(…)` to `src/commands/<area>.rs` (a new area also goes in
   `commands/mod.rs`).
3. Register it in `generate_handler!` in `lib.rs`.
4. Run `pnpm types` if it returns a new type, then add its function to `src/lib/backend.ts` with a fake answer.

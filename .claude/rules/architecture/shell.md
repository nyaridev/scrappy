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
- `events.rs` turns core's job reports into the `job-progress` and `job-finished` events.
- A command that starts a job is `async`: `Jobs::start` needs the Tokio runtime async commands run in.
- Commands live in `src/commands/<area>.rs`, one file per view or system, listed in `commands/mod.rs`.
- A command parses its arguments, makes one `scrappy_core` call and returns the result. Any `if` about behavior
  belongs in core.
- Long work: the command starts a core job and returns its id. Progress is forwarded as `job-progress`, the result as
  `job-finished` (ADR-0010). No other event names without an ADR change.
- Return types come from core with `ts_rs` exports; the shell defines no types the window sees.
- Errors go back as `Result<T, String>` with a readable message.

## Capabilities and config

- `capabilities/default.json` grants `core:default` and the window-control permissions from ADR-0011, nothing else.
  Adding any plugin permission (fs, shell, http, dialog, opener…) to the window needs an ADR-0005 change. Plugins Rust
  calls itself (a folder picker opened from Rust) need no window permission.
- Keep the CSP in `tauri.conf.json` strict. Loosen one source at a time, and only for something the window loads.
  Library pictures reach the window as `data:` URLs (ADR-0004, M9), so they need no asset protocol.
- Release builds merge `tauri.release.conf.json`: bundling on, the `scrappy-host` sidecar staged by
  `scripts/stage-host.mjs` (ADR-0014). Keep `tauri.conf.json` free of the sidecar, so development needs no host build.
- Dev server port: 4430 (`vite.config.ts`, `tauri.conf.json`, `.claude/launch.json`, `start-dev.bat`).

## How to add a command

1. Write the logic in core first, with its tests.
2. Add `#[tauri::command] pub fn <name>(…)` to `src/commands/<area>.rs` (a new area also goes in `commands/mod.rs`).
3. Register it in `generate_handler!` in `lib.rs`.
4. Run `pnpm types` if it returns a new type, then add its function to `src/lib/backend.ts` with a fake answer.

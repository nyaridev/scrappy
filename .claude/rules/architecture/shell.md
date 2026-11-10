---
paths:
  - "src-tauri/**"
---

# Tauri shell

`src-tauri/` opens the window, registers commands and forwards progress. It holds no logic.

- `lib.rs` wires things up: setup (logging, state), plugins, `generate_handler!`. Nothing else.
- Commands live in `src/commands/<area>.rs`, one file per view or system, listed in `commands/mod.rs`.
- A command parses its arguments, makes one `scrappy_core` call and returns the result. Any `if` about behavior
  belongs in core.
- Long work: the command starts a core job and returns its id. Progress is forwarded as `job-progress`, the result as
  `job-finished` (ADR-0010). No other event names without an ADR change.
- Return types come from core with `ts_rs` exports; the shell defines no types the window sees.
- Errors go back as `Result<T, String>` with a readable message.

## Capabilities and config

- `capabilities/default.json` grants `core:default` only. Adding any plugin permission (fs, shell, http, dialog,
  opener…) to the window needs an ADR-0005 change. Plugins Rust calls itself (a folder picker opened from Rust) need
  no window permission.
- Keep the CSP in `tauri.conf.json` strict. Loosen one source at a time, and only for something the window loads
  (the asset protocol for gallery thumbnails, M9).
- Dev server port: 4430 (`vite.config.ts`, `tauri.conf.json`, `.claude/launch.json`, `start-dev.bat`).

## How to add a command

1. Write the logic in core first, with its tests.
2. Add `#[tauri::command] pub fn <name>(…)` to `src/commands/<area>.rs` (a new area also goes in `commands/mod.rs`).
3. Register it in `generate_handler!` in `lib.rs`.
4. Run `pnpm types` if it returns a new type, then add its function to `src/lib/backend.ts` with a fake answer.

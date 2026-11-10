---
paths:
  - "crates/scrappy-core/**"
  - "packs/**"
---

# scrappy-core

Everything Scrappy does lives here. It never uses Tauri: it takes plain arguments (paths, URLs, settings) and returns
results, and reports progress through a callback the shell passes in.

## Modules

| Module | Owns | ADR |
|---|---|---|
| `library` | the root folder, its layout, path templates, the catalog (`.scrappy/library.db`) | 0004 |
| `jobs` | the queue, cancel flags, concurrency, progress callbacks | 0010 |
| `tools` | finding, downloading, checking and updating yt-dlp, Deno, gallery-dl, ffmpeg, ffprobe | 0002 |
| `media` | probing and downloading with yt-dlp and gallery-dl, encoding with ffmpeg, presets | 0002, 0003 |
| `grab` | finding a page's media (extension finds, CDP deep scan), classifying, joining streams | 0007 |
| `crawl` | crawling with limits, offline mirrors, single-page saves | 0008 |
| `hunt` | query detection, source packs, collecting results | 0009 |
| `dedupe` | perceptual hashes, picking the best copy, moving the rest to the trash | 0009 |
| `vault` | cookies and secrets, the keychain key, temporary cookie files | 0005 |
| `bridge` | the extension's message format and its validation | 0006 |
| `settings` | the app settings file | 0004 |
| `logging` | the log file and its redaction | 0005 |

- Modules call each other only through `pub` functions; no reaching into another module's internals.
- External programs run only through `tools` (it knows their paths) and are driven only from `media`, `grab` and
  `crawl`.
- Functions return `Result<T, String>` with a message a person can read, until a module needs its own error type.
- Every module with logic has tests in `#[cfg(test)] mod tests`; anything that touches disk uses a temp folder.

## How to add a system

1. Write `crates/scrappy-core/src/<name>.rs` (header first), and add `pub mod <name>;` to `lib.rs`.
2. Long work runs as a job (`jobs`), never by blocking a command.
3. Types the window sees derive `Serialize`, `ts_rs::TS` with `#[ts(export)]` and `#[serde(rename_all = "camelCase")]`.
4. Add a row to the table above, and to `docs/adr/README.md` if it changes the module table there.

## How to add a site

1. If yt-dlp or gallery-dl supports it, add nothing but tests: `media` already routes any URL to them.
2. Otherwise add a driver file under `media/` (or `grab/` for page scans) and route its domain to it in one place.

## How to add a preset or a source pack

- Preset: one entry in the built-in presets in `media` (ADR-0003). Presets are data; never branch on a preset's name.
- Source pack: a TOML file in `packs/` (ADR-0009). A new kind of source is a module in `hunt/sources/`.

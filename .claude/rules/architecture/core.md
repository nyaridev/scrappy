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
| `library` | the root folder, its layout, path templates, the catalog (`.scrappy/library.db`) and what it says was downloaded | 0004 |
| `jobs` | the queue, cancel flags, concurrency, progress callbacks | 0010 |
| `tools` | finding, downloading, checking and updating yt-dlp, Deno, gallery-dl, ffmpeg, ffprobe; finding the installed browser | 0002, 0012 |
| `media` | probing and downloading with yt-dlp and gallery-dl, encoding with ffmpeg, presets, the Gallery's thumbnails, the Download list (`downloads.db`), download slots and the speed limit | 0002, 0003, 0016, 0017 |
| `grab` | finding a page's media (extension finds, CDP deep scan), classifying, the handed-page inbox, saving picks through `media` | 0007 |
| `crawl` | crawling with limits, offline mirrors, single-page saves | 0008 |
| `hunt` | source packs, the web search, the content-image finder, saving what it finds | 0013 |
| `dedupe` | perceptual hashes, picking the best copy, moving the rest to the trash | 0009 |
| `vault` | cookies and secrets, the keychain key, temporary cookie files | 0005 |
| `bridge` | the extension's message format, its validation, and the app's local socket | 0006 |
| `settings` | the app settings file | 0004 |
| `logging` | the log file and its redaction | 0005 |

- Modules call each other only through `pub` functions; no reaching into another module's internals.
- External programs run only through `tools` (it knows their paths) and are driven only from `media`, `grab` and
  `crawl`.
- Functions return `Result<T, String>` with a message a person can read, until a module needs its own error type.
- Every module with logic has tests in `#[cfg(test)] mod tests`; anything that touches disk uses a temp folder.

## How to add a system

1. Write `crates/scrappy-core/src/<name>.rs` (header first), and add `pub mod <name>;` to `lib.rs`.
2. Long work runs as a job (`jobs`), never by blocking a command. A download that shows in the Download list takes
   a slot from `Downloads`' queue (a playlist, one per video), so "Downloads at once" holds for all of them
   (ADR-0017); crawls and hunts fetch inside their own job.
3. Types the window sees derive `Serialize`, `ts_rs::TS` with `#[ts(export)]` and `#[serde(rename_all = "camelCase")]`.
4. Add a row to the table above, and to `docs/adr/README.md` if it changes the module table there.

## How to add a site

1. If yt-dlp supports it, add nothing but tests: `media` sends every link to it. If gallery-dl does it better
   (posts, profiles, image galleries), add the site to `GALLERY_SITES` in `media/route.rs`.
2. Otherwise add a driver file under `media/` (or `grab/` for page scans) and route its domain to it in one place.

## How to update a tool's pin

Change its entry in `tools/manifest.rs`: the release, the URLs, and each file's SHA-256 from GitHub's API (the asset's
`digest`), all in one commit. FFmpeg pins a BtbN month-end `autobuild-*` release, never a daily one (ADR-0012).

## How to add a preset or a source pack

- Preset: one entry in the built-in presets in `media` (ADR-0003). Presets are data; never branch on a preset's name.
- Source pack: a TOML file in `packs/` listing sites (ADR-0013). Packs are data: every site goes through the same
  web search and content-image finder; never add code for one site. Another search engine is a module beside
  `hunt/search.rs`.

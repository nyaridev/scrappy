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
| `library` | the root folder, its layout, path templates, the catalog (`.scrappy/library.db`) and what it says was downloaded, the Gallery's pages and folders, deleting a file or downloaded folder for good, streaming video and audio to the player, moving the library to another folder as a job | 0004, 0015, 0021, 0022, 0023, 0039 |
| `jobs` | the queue, cancel flags, concurrency, progress callbacks | 0010 |
| `tools` | finding, downloading, checking and updating yt-dlp, Deno, gallery-dl, ffmpeg, ffprobe; finding the installed browser | 0002, 0012 |
| `media` | probing and downloading with yt-dlp and gallery-dl, the window's link check that lists as it comes (`probes`, `listing`), what the extension's page is (`Probes::look`, yt-dlp's generic reader off), naming a sent stream after its page, encoding with ffmpeg, presets, the Gallery's thumbnails, the Download list (`downloads.db`, with rows for scrapes and hunts through `track`), download slots and the speed limit | 0002, 0003, 0016, 0017, 0018, 0030, 0042 |
| `grab` | finding a site's media (extension finds, CDP deep scan as a job, following its site's links to a depth), classifying, find thumbnails, the handed-page inbox, saving picks through `media`, saving finds as they're found (`keep.rs`), removing a grab's duplicates through `dedupe` | 0007, 0019, 0029, 0031, 0032 |
| `crawl` | crawling with limits, offline mirrors (pages first, each mirror's record in `.scrappy/mirrors/`, picked media saved after through `picks`), single-page saves | 0008, 0031, 0033 |
| `hunt` | source packs (saving one edited in the window as the user's, creating and removing the user's own), the web search, the content-image finder, saving what it finds, each source's progress (`Hunts`), image search engines (`engines.rs`), which also find a site's images, removing a hunt's duplicates through `dedupe` | 0013, 0020, 0024, 0025, 0026, 0027, 0028 |
| `dedupe` | perceptual hashes, picking the best copy, moving the rest to the trash (after a review, or after a hunt or a grab with strict matching) | 0009, 0027, 0029 |
| `vault` | cookies and secrets, the keychain key, temporary cookie files | 0005 |
| `bridge` | the extension's message format, its validation, and the app's local socket | 0006 |
| `settings` | the app settings file, the preferences and their checks | 0004, 0034 |
| `net` | the proxy every request goes through, and the HTTP clients that use it (`net::client()`, never `Client::builder()`) | 0034 |
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
   (posts, profiles, image galleries), add the site to `GALLERY_SITES` in `media/route.rs`. When yt-dlp reads its
   single posts' videos with their qualities (X), add the post path to `VIDEO_FIRST` too: yt-dlp is asked first, and
   gallery-dl when there's no video.
2. Otherwise add a driver file under `media/` (or `grab/` for page scans) and route its domain to it in one place.

## How to update a tool's pin

Change its entry in `tools/manifest.rs`: the release, the URLs, and each file's SHA-256 from GitHub's API (the asset's
`digest`), all in one commit. FFmpeg pins a BtbN month-end `autobuild-*` release, never a daily one (ADR-0012).

## How to add a preset or a source pack

- Preset: one entry in the built-in presets in `media` (ADR-0003). Presets are data; never branch on a preset's name.
- Source pack: a TOML file in `packs/` listing sites (ADR-0013). Packs are data: every site goes through the same
  image search and content-image finder (ADR-0026); never add code for one site. Another web search engine is a module beside
  `hunt/search.rs`; another image search engine is one `ImageEngine` value and its reader in `hunt/engines.rs`, with a
  test on a saved sample of its results (ADR-0025).

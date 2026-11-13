# ADR-0004: The library folder and its catalog

**Status:** Accepted; the Gallery reads the folder, not the catalog: [ADR-0015](0015-gallery-from-the-library-folder.md);
the settings file holds the preferences: [ADR-0034](0034-preferences.md)
**Date:** 2026-11-10

## Context

On first launch the user picks where everything goes. A folder that isn't empty needs a warning, then Scrappy creates
its folders. Home shows stats for 24h, 7d, 30d and all time (files, videos, total length, bytes transferred) and the
recent downloads, and the gallery browses everything. Downloads should sort themselves by channel, playlist or site.
Per-item folders, metadata and thumbnails are options, off by default.

## Decision

- **Layout** inside the chosen root, created on setup: `Videos/<channel>/[<playlist>/]`, `Audio/<channel>/`,
  `Social/<site>/<user>/`, `Grabs/<domain>/<date>/`, `Images/<topic>/`, `Sites/<domain>/` and `.scrappy/`.
- **Path templates** decide the names (`{title} {id} {channel} {playlist} {upload_date} {site} {user}`), sanitized and
  capped at 120 characters, as in the old `video-downloader`. "One folder per item, with metadata and thumbnail" is an
  option.
- **Catalog:** SQLite (`rusqlite`, bundled) at `<root>/.scrappy/library.db`, with one row per downloaded file: path,
  kind, source URL, site, size, duration, codec, bytes transferred and timestamps. Home stats and the gallery are
  queries on it. Schema changes are numbered migrations in `library/`.
- **Settings** (the root path, presets, the look) live in the app config folder, not in the library.

## Options considered

- **Catalog in the app data folder:** lost on a reinstall or another PC, so the library wouldn't be portable.
- **No catalog, scan the disk:** no transfer stats, no source URLs, and slow on big folders.

## Consequences

- **Easier:** move or back up the folder and the stats and gallery come with it.
- **Harder:** files added or removed by hand need a rescan to match the catalog.
- **Later:** a new download kind is a folder name, a template and a `kind` value. Settings can change the library
  folder while Scrappy runs (M15): point at another library, or move this one with its catalog.
- **Rules:** `.claude/rules/architecture/core.md`.

## Implementation notes (M9)

- The Gallery reads the catalog a page at a time (`library::page`): filters by kind, site and a search on the path,
  sorts by date, size or name, and marks files gone from disk.
- Thumbnails are small JPEGs in `.scrappy/thumbs/`, named by a hash of the file's path and made again when the file
  is newer. `media` makes them with FFmpeg (only `media`, `grab` and `crawl` run tools), three at a time. They reach the
  window as `data:` URLs, so the CSP stays strict and the asset protocol isn't used; a picture opened large travels
  the same way, up to 30 MB, and anything else opens in its app.

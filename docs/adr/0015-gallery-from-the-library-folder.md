# ADR-0015: The Gallery shows the library folder, watched live

**Status:** Accepted
**Date:** 2026-11-10
**Supersedes:** ADR-0004's Gallery reading the catalog (M9); adds an event to ADR-0010

## Context

The Gallery lists the catalog's rows (M9). A file deleted or moved by hand stays as a "missing" tile, a file added by
hand never shows, and nothing changes until the view is opened again. Nari wants the Gallery to match what is in the
Scrappy folder, and to update by itself while it's open. Home's numbers are a history of downloads and stay on the
catalog.

## Decision

- **The Gallery lists the files on disk** in the library folder, walking it in `library/disk.rs`. Skipped: hidden
  folders and files (`.scrappy/` and the like) and downloads in progress (`.part`, `.ytdl`, `.tmp`). Under `Sites/`
  only a mirror's start page (`Sites/<site>/index.html`) and saved pages (`Sites/<site>/_pages/*.html`) count; the
  rest are a mirror's own files.
- **The catalog adds what it knows** about a file by its path: its kind (`social`), site, account, length and
  download date. A file the catalog doesn't know gets its kind from its extension and its date from the disk.
- **A watcher** (`notify`) on the library folder marks the listing stale when anything outside the skipped files
  changes, and tells the window with one new event, `library-changed` (no payload, debounced half a second). The
  Gallery reloads the rows it shows when it hears it; closed, it reads fresh on its next open. The watcher starts with
  the first Gallery page and follows the library folder if it changes.
- The type and site filters list what the folder holds, from the same listing.

## Options considered

- **Keep the catalog and mark missing files** (M9): no folder walk, but hand-added files never show.
- **The window polls every few seconds:** no new event, but constant work, and a lag.
- **Delete catalog rows of removed files:** Home's history would shrink, which Nari doesn't want.

## Consequences

- **Easier:** the Gallery always matches the folder, whatever changed it.
- **Harder:** a walk of the whole folder after each change (fine to tens of thousands of files); watchers can miss
  events on network drives, where the next open still reads fresh.
- **Later:** duplicate removal still looks at the catalog's pictures.
- **Rules:** `.claude/rules/architecture/shell.md`, `.claude/rules/architecture/frontend.md`.

## Implementation notes

- `library/disk.rs` walks the folder without following linked folders and keeps the list until the watcher
  (`notify-debouncer-mini`, half a second) sees a change the Gallery would show; changes in `.scrappy/` (the catalog,
  thumbnails) and partial downloads don't count.
- A refresh reads again every row the window shows (up to 1000), so the grid keeps its place.
- `GalleryItem.missing` is gone: every row is a file on disk.

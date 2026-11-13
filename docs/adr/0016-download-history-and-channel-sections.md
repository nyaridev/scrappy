# ADR-0016: The Download list is kept; a channel's lists are picked one by one

**Status:** Accepted; where the list shows is superseded by ADR-0018 (the Downloads view); lists load as they come
(ADR-0030)
**Date:** 2026-11-11
**Supersedes:** the README's "The Download list lasts one session" (Later); ADR-0002's probe listing 50 entries

## Context

The Download list lives in memory and empties when Scrappy closes. Nari wants it kept, files deleted or not, with a
way to remove rows and clear it. A YouTube channel link (`youtube.com/@name`) probes as two entries, "Videos" and
"Shorts": yt-dlp returns a channel's tabs as lists inside the list. The window says "2 videos", and there's no way
to take the videos without the shorts.

## Decision

- **The list is kept** in `downloads.db` (SQLite) in the app's data folder, one row per download, written when a
  download is added, changes state, or is removed. A download that was queued or running when Scrappy closed shows
  as stopped. It isn't tied to the files: a row stays after its file is deleted.
- **Removing:** a finished row (done, failed, stopped) has a remove button; "Clear" removes every finished row.
  Queued and running rows are stopped first, with their Stop button. Removing a row never touches its files.
- **Ids stay unique across starts:** job ids continue after the highest kept id, so a new job can't take an old
  row's id (ADR-0010's ids stay the download's id).
- **Sections:** when a yt-dlp listing holds lists (each entry a `playlist` with its own link, as a channel's tabs
  are), the probe reports them as sections, each with its own count; the link's count is their sum. The window shows
  one checkbox per section, all on. Each picked section is queued as its own download (its own row and folder), from
  the probe's own listing of it, so nothing is asked again.
- **Picking videos:** every list (a playlist, or each section) carries its videos as listed: link, title, length,
  and whether its thumbnail is tall (a short). The window shows each list as a card with its first thumbnails;
  clicking it opens a pop-up grid of the list's videos as thumbnail cards that pick or drop on a click, with a
  switch between a channel's lists. Thumbnails come from Rust as `data:` URLs (ADR-0005), one per card as it scrolls
  into view (`entry_thumbnail`), six fetched at once; Rust only fetches thumbnails of videos a probe listed. The links turned off go with the download; when any are off, yt-dlp gets a temporary file of the picked
  links (`-a`) instead of the list's link, saved into the same folder.
- **The checked result stays** while the link field is edited; Enter or the arrow checks the new link.
- **Real counts:** yt-dlp probes list up to 1000 entries (was 50), so a channel's count is exact up to there; past
  that it shows "1000+". gallery-dl keeps its 50-file listing (its sites limit request rates), and already counts
  each file of a profile's parts.

## Options considered

- **The list in the library's catalog:** the catalog is per library and holds files, not attempts; a failed
  download has no file. The app's data folder keeps the list across library changes.
- **A JSON file:** each download carries a thumbnail; rewriting every row on each change grows with the list.
- **Probing each section again on download:** simpler, but slower, and the count could change between probe and
  download.
- **One job for all picked sections:** one row, but one folder name for two lists, and no way to stop just one.

## Consequences

- **Easier:** the list is there after a restart; a channel's videos and shorts are counted and picked apart.
- **Harder:** a channel probe takes a few seconds longer (about 6 s for 550 entries).
- **Later:** sections for gallery-dl profiles (Instagram posts, reels, stories) through its `include` option.
- **Rules:** `.claude/rules/architecture/core.md`, `.claude/rules/architecture/frontend.md`.

## Implementation notes

- `media/download_list.rs` holds the rows; `Downloads::kept_in(dir)` loads them at start, and `Jobs::continue_after`
  takes the highest kept id. Tests keep using `Downloads::new`, whose list stays in memory.
- `remove_download` and `clear_downloads` return the list as it is after the change; Rust refuses to remove a queued
  or running download.
- The pop-up is `src/features/download/VideoPicker.tsx` (iro-ui `Dialog`, `Segmented`) with `VideoCard.tsx`; the
  list cards are `ListTiles.tsx`; `usePreview.ts` asks for a thumbnail when its card shows. A list cut at 1000 entries with some turned off downloads only the listed ones it kept.
- `media/probe.rs` reads the nested lists (`_type: playlist` with a `webpage_url`) as sections, titled without the
  channel's `"<name> - "`. Tried on a real channel: `@GDULA7` probes as Videos 6 and Shorts 16 (22), and
  `@veritasium` (453 and 96 entries) lists in about 6 seconds (the ignored `counts_a_channels_videos_and_shorts`).

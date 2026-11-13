# ADR-0030: A link's list loads as it comes, and can be stopped

**Status:** Accepted
**Date:** 2026-11-12
**Amends:** ADR-0002 and ADR-0016 (a link was checked by one `yt-dlp -J` and shown when all of it was listed)

## Context

Checking a link waits for yt-dlp's whole answer. A playlist or Mix of hundreds of videos takes 15 to 20 seconds
before anything shows, and there's no way to stop it. Nari wants the list to show from its first video, grow in
place without moving the scroll, take more "First N" amounts, and stop when it has enough. The picker's grid of
hundreds of cards needs pages, as Grab's results have.

## Decision

- **Checking a link is a job** (`list_link`), as Grab's scan is (ADR-0019): its progress is the videos listed so far,
  and the window reads the probe so far (`listed_link`) as reports come, in batches. Only the latest check is kept.
- **yt-dlp lists lazily:** `--flat-playlist --lazy-playlist -j` prints each entry as its page of the list arrives
  (the first in about 2 seconds), with its list's title, link, channel and count when the site gives one. Rust groups
  the lines by list link and builds the same listing `-J` gave: one list, or a channel's lists as sections. A link
  that's one video prints one line, its whole JSON, as before. gallery-dl's listings stay one answer.
- **While listing**, a list without a known count shows "N+ videos"; Download works at once: without videos turned
  off it downloads from the list's link (with the "First N" limit), so nothing waits for the listing.
- **Stop** ends the listing and keeps what was listed: that becomes the list, counted as listed.
- **Amounts:** First 10, 25, 50, 100, 150, 200, 300 and 500.
- **The picker shows 100 cards a page**, with iro-ui's `Pagination`; Select all and none act on every card the
  filters show, not one page.
- A list's card takes its first video's thumbnail when the list has none (a Mix).

## Options considered

- **`-J` in growing batches (`--playlist-items 1-25`, then all):** every batch asks the site again from the start;
  the list grows in two jumps, not as it comes.
- **Plain `-j` without `--lazy-playlist`:** yt-dlp gathers the whole list (to number it) before printing the first
  entry; tried on a Mix, the first line came after 21 seconds.

## Consequences

- **Easier:** a long list shows in seconds and can be cut short; a download can start before the listing ends.
- **Harder:** a list's own thumbnail and the channel's own title aren't in lazy output: the first video's thumbnail
  and the channel's name stand in.
- **Rules:** `.claude/rules/architecture/core.md`, `.claude/rules/architecture/frontend.md`.

## Implementation notes

- `media/listing.rs` reads the lines into the listing; `media/probes.rs` (split out of `downloads.rs`, which passed
  1000 lines) holds the probes kept for downloads, the running check, thumbnails and size estimates.
  `Probes::list_link` starts a check and `Probes::listed_link` returns its probe so far; the window follows it in
  `src/features/download/useLinkCheck.ts`. A download asked for while listing uses the check's listing so far. A
  kept probe says whether its listing was whole; only a whole one downloads video by video from the listed links
  when nothing is turned off.
- Tried on the Mix `watch?v=Tx8Ml-k4Z-8&list=RDdgKjUrX5YKs`: the first video after 1.7 s, 449 listed in 20 s.

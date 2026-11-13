# ADR-0019: Grab's scan runs as a job and shows its finds as they arrive

**Status:** Accepted (a scan follows linked pages since ADR-0029)
**Date:** 2026-11-12
**Amends:** ADR-0007's deep scan (how it runs and reaches the window; what it finds stays the same)

## Context

A deep scan takes up to 40 seconds (the page's load, then 8 seconds of play), and the window shows only a spinner
until it ends. Nari wants to see what the scan has found while it runs, and the list of finds to grow as it does.

## Decision

- **The scan is a job** (ADR-0010). `start_scan` returns its id; `grab::Scans` holds what each scan has found so far.
  Every half second the scan sorts what it has seen into finds, keeps them there, and reports the count as `items`
  progress. No new event: the window reads the finds with `scanned_page(id)` when a report arrives, and once more
  when the job ends.
- **Stop** cancels the job. The scan ends early and keeps what it found, so the window still shows those finds.
- Only the latest scan is kept: a new one drops the others.
- The window shows each scan's finds in the grid or list as they arrive. Its filter, picks and page reset when a new
  scan starts, not on each update.
- **Thumbnails** of finds are made when the window shows them (`find_thumbnail`): a picture scaled down, a video's or
  stream's frame, read by FFmpeg from the web with only web protocols allowed.

## Options considered

- **A new event carrying the finds:** pushes up to 2000 finds every half second whether or not Grab is open; the
  job's count and a read when needed cost less and need no new event.
- **Showing only a count:** simpler, but the finds would still appear all at once at the end.

## Consequences

- `scan_page` is gone; the extension's handed page (ADR-0007) is unchanged.
- A finished scan's finds stay in core until the next scan starts.

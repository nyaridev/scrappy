# ADR-0018: One Downloads list, opened from the header

**Status:** Accepted
**Date:** 2026-11-12
**Supersedes:** ADR-0016's Download list living under the Download view (the list itself and how it's kept stay)

## Context

The kept Download list sits under the Download view, and holds only what the Download view, Grab and the extension
started. Scrapes and hunts save files too, but each shows only in its own view while it runs. Nari wants every
download in one list, filtered by where it came from, opened from an icon in the header, with the Download view
showing only its last three.

## Decision

- **A Downloads view** opens from an icon button (`TrayInIcon`) left of Settings, like Settings: not a tab. It shows
  the whole list, newest first, with a filter: All, Download, Grab, Scrape, Hunt, Extension. "Clear all" removes the
  finished rows the filter shows.
- **Every row has a source**: `download`, `grab`, `scrape`, `hunt` or `extension`, set where it's queued. Rows kept
  from before have none and read as `download`.
- **Scrapes and hunts get rows.** `Downloads::track` adds a row for a job another module runs and starts it outside
  the download slots (a crawl or hunt keeps its own limits, ADR-0008, ADR-0013). The row shows the site or the
  search, the mode ("Site files", "Mirror", "Page", "Images"), the files saved and the folder; the job returns how
  many files it saved. A hunt's row has no link. Their views still follow their own jobs by id, as before.
- **The Download view** shows its own ten latest rows (source `download`; three until 2026-11-12) under a "Last 10
  downloads" heading, and an "All downloads" button that opens the Downloads view.
- **Files kept as they are name their types** (added 2026-11-12): a gallery-dl download, a picked file or an image
  from the extension shows its saved files' extensions in the preset's place, the three most common first (`MP4`,
  `JPEG, MP4, PNG`), not "Original files". A playlist or profile that saved one file and skipped none reads as that
  file, with its size and path, as a single video does. Rows kept from before show the preset as they did.

## Options considered

- **A pop-up from the header icon:** quick to glance at, but a thousand rows and a filter need a page's room, and a
  view keeps its scroll and filter when it's left.
- **Filters by site (YouTube, X…):** Nari's examples are the views; the site is in each row's link and title.

## Consequences

- The list module (`useDownloads`) and the rows (`DownloadQueue`) are shared by three views, so they move to
  `src/app/` and `src/components/`.
- Scrape and hunt rows carry no size: their files are spread across a folder that earlier runs also wrote to.

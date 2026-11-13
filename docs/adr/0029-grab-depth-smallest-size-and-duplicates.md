# ADR-0029: Grab scans linked pages, hides small finds and removes duplicates

**Status:** Accepted; Grab saves as found since ADR-0031, which also raised the scan to 10,000 pages, six tabs at
a time; the page limit is no longer a setting since ADR-0032; how strictly copies are removed is a setting since
ADR-0034
**Date:** 2026-11-12
**Amends:** ADR-0007 and ADR-0019 (a deep scan looked at the one page it was given)

## Context

Grab's deep scan opens one page in a headless browser and lists what it can download (ADR-0007, ADR-0019). A gallery
spread over several pages means scanning each by hand, the finds include icons and tracking pixels, and saving a
page's pictures often brings the same picture at several sizes. Nari wants a depth, a smallest size, and duplicates
removed by default, as Scrape and Hunt have.

## Decision

- **Depth and Pages:** a scan request has a depth (1, the page alone, to 5; 1 by default, so a scan is what it was)
  and the most pages it looks at (1 to 100; 20 by default). Past the first page it follows the page's links on the
  same site (`vault::site_of`) that aren't finds themselves, level by level, three tabs at a time in the one headless
  browser, each page loaded, played and given the same settle time as the first. Every page's finds are merged into
  one list; the scan reports how many pages it has looked at. Stop ends it with what it has.
- **Smallest (KB):** the window hides finds the page said are smaller (10 KB by default), as it filters by type: it
  applies at once and needs no new scan. A find of unknown size stays, since nothing says it's small.
- **Remove duplicates:** a switch beside Download, on by default. When on and two or more pictures were picked, Grab
  adds one more job to the Download list once the picks are queued: it waits for those downloads to end, then moves the
  grab folder's extra copies to the trash with strict matching, keeping each picture's best copy (ADR-0009, as Hunt does
  in ADR-0027). Its row counts the copies it removed. The folder is the day's `Grabs/<domain>/<date>/`, so an earlier
  grab from the same site that day is looked at too.

## Options considered

- **Depth through plain page fetches (Scrape's crawler):** fast, but misses what only a browser sees: streams,
  lazy images, requests made by players. Grab exists for those.
- **Smallest size in the scan:** finds below it would be gone until the next scan; the filter is display only.
- **Removing duplicates per download as it ends:** a copy saved later would undo the pick; one pass at the end sees
  them all.

## Consequences

- **Easier:** a gallery's pages in one scan; fewer tiny finds to wade through; no second pass for duplicates.
- **Harder:** a deeper scan takes as long as its pages do (about ten seconds each, three at a time).
- **Rules:** `.claude/rules/architecture/core.md`.

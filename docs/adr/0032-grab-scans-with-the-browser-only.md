# ADR-0032: Grab scans with the browser only, to a depth

**Status:** Accepted
**Date:** 2026-11-12
**Supersedes:** ADR-0031's fast scan; ADR-0029's page limit as a setting

## Context

ADR-0031 gave Grab two engines: *Browser* (a headless browser that sees everything a page loads) and *Fast* (plain
page fetches that see only the HTML's links). Once the browser scan reached the same depth and page limits, Fast
was a second choice that found less, and the engine tabs and their extra field (Delay) made Grab's card busier.
The Pages field was greyed out at depth 1, which is where most scans start, and with depth already bounding a scan
it was one more number to set. Nari wants both gone.

## Decision

- **One engine:** every Grab scan runs in the headless browser (ADR-0007, ADR-0031's quiet settle and six tabs).
  `ScanEngine`, `ScanRequest.engine` and `delay_ms`, and `grab/fast.rs` are removed.
- **Depth is the only limit the window sets** (1 to 10). The scan looks at every page on the site it reaches within
  that depth, up to 10,000 pages, which Rust holds to; `ScanRequest.pages` is removed. Stop ends a scan early with
  what it found.
- **Scrape keeps Mirror site and Save page** (ADR-0031); crawling a site for its files without a browser is gone.

## Options considered

- **Keep Fast, hide Pages only:** keeps a scan that misses what scripts load, for speed most grabs don't need.
- **Keep Pages, enabled at any depth:** one more number, when Stop already ends a scan that runs too long.

## Consequences

- **Easier:** Grab's card has the link, Depth, Smallest and the saving options; every scan finds the same things.
- **Harder:** a deep scan of a big site runs until it has looked at every page within the depth (up to 10,000),
  which can take hours: Stop is the way to end it early. A site that only plain fetches reach (one that blocks
  headless browsers) can't be scanned to a depth.
- **Rules:** `.claude/rules/architecture/core.md`.

## Implementation notes

- `grab/scan.rs` keeps `MAX_PAGES` as its own cap. `crawl::crawler` folded back into `crawl/request.rs`, and
  `crawl::file_links` went with Fast; `crawl/links.rs` lists only a mirror's assets.

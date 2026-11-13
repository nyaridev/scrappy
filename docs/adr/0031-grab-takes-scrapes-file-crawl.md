# ADR-0031: Grab takes Scrape's file crawl; Scrape keeps whole sites and pages

**Status:** Accepted; the fast scan was removed by ADR-0032, and Grab scans with the browser only
**Date:** 2026-11-12
**Amends:** ADR-0008 (Scrape's "Files" mode), ADR-0007 and ADR-0029 (Grab scans with a browser only, and saves only
what's picked)

## Context

Grab finds a site's files across its linked pages (ADR-0029), and so does Scrape's Files mode, which crawled with
plain page fetches and saved everything of the kept kinds. Two views did nearly the same job: one thorough (a real
browser) and hands-on (pick, then download), the other fast and in bulk. Nari wants one place for a site's assets,
Grab, and Scrape for everything else: a whole site to read offline, or one page with all it needs.

## Decision

- **Grab scans with a browser or fast,** both up to depth 10 and 10,000 pages. *Browser* is the deep scan: each page
  runs until it has had no new response for 1.5 seconds (8 at most, as before), six pages at a time. *Fast* is
  Scrape's crawler: plain fetches of the site's pages with a delay between requests, following `robots.txt`; each
  page's links to files become finds. Both show their finds in Grab's grid and list as they come.
- **One entry per link:** a scan keeps each file it has seen once, however many pages show it, so thousands of pages
  don't pile up millions of requests. The window lists the first 2,000 finds by type; saving as found takes every
  one.
- **Download as found:** a switch, off by default. On, the scan saves each find of the kept kinds (images, videos,
  audio, other files; streams need picking, as they go through yt-dlp) into `Grabs/<domain>/<date>/`, four at a
  time, up to a number of files (up to 10,000), skipping links the library has and files under Smallest. Each tile
  shows its find's state: Saving, Saved, Too small, Already saved or Failed. The scan gets one row in the Download
  list counting the files it saved, and with Remove duplicates on it removes the folder's extra copies once it ends
  (ADR-0029). Finds that weren't saved can still be picked and downloaded.
- **Scrape keeps two modes:** Mirror site and Save page. Its Keep and Smallest options go with Files.

## Options considered

- **Keep both, with a results grid in Scrape:** two views that list a site's files, with different options and
  folders; the grid would be built twice.
- **Fast scans only in Scrape, Grab browser-only:** the split Nari found confusing.

## Consequences

- **Easier:** one place to find and save a site's files, fast or thorough, picked or all of them; what a bulk save
  took is visible file by file.
- **Harder:** Grab's card has more options; they show only when they apply (Delay for Fast, the Keep tray and Files
  when saving as found). A browser scan of thousands of pages still takes hours (about a second and a half a page at
  best, six at once), and six tabs use more memory than three.
- **A page that keeps loading** (a live feed, an ad loop) never goes quiet and gets the full 8 seconds.
- **Files land in `Grabs/<domain>/<date>/`,** no longer `Images/<domain>/`; earlier crawls' files stay where they are.
- **Rules:** `.claude/rules/architecture/core.md`.

## Implementation notes

- `grab/fast.rs` is the crawl that was `crawl/files.rs`, now listing finds; it uses `crawl::crawler` (the `spider`
  setup Mirror shares) and `crawl::file_links`. `grab/keep.rs` saves finds as they come through
  `Downloads::save_now`, for both scans. `find::Found` holds a scan's finds by link. `CrawlMode::Files`,
  `CrawlRequest.kinds` and `min_kb` are gone.
- Tried with the ignored browser tests: the two w3schools pages and the HLS demo scan in about 10 seconds (16 with
  the fixed wait), still finding the video and the stream.

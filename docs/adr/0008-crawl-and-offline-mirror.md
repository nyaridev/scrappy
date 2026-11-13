# ADR-0008: Crawl and offline mirror

**Status:** Accepted; crawling a site for its files moved to Grab (ADR-0031), which scans with the browser only (ADR-0032); a mirror's
images and videos are picked after its crawl (ADR-0033)
**Date:** 2026-11-10

## Context

Scrape needs two kinds of crawl: collecting images (or other files) from a site within limits (pages visited, files
kept, depth), and mirroring a site (HTML, CSS, JS, images) so it works offline. No maintained Rust tool rewrites links
for a whole-site offline copy.

## Decision

- **Crawling:** the `spider` crate, in HTTP mode first and Chrome mode for script-heavy sites. Depth, page budget,
  same-site scope, delay and `robots.txt` are respected by default, and every limit is a field on the job.
- **Mirror:** spider fetches, then our own pass with `lol_html` rewrites `href`, `src`, `srcset` and CSS `url()` to
  relative local paths, into `Sites/<domain>/`.
- **One page as one file:** the `monolith` crate (HTML with everything inlined).
- Image crawls feed the same download jobs and duplicate removal as Hunt (ADR-0009).

## Options considered

- **wget or HTTrack as another external tool:** GPL binaries whose progress is hard to read reliably.
- **`suckit`:** rewrites links, but has been abandoned since 2022.

## Consequences

- **Easier:** limits and politeness are built in, and one crawler serves both images and mirrors.
- **Harder:** the link rewrite is ours to maintain, and dynamic sites mirror only as well as their HTML allows.
- **Later:** a new crawl target (PDFs, audio) is a filter on the same crawler.
- **Rules:** `.claude/rules/architecture/core.md`.

## Implementation notes (M11)

- `spider` 2.53 with its default features off (no SQL cache, no io_uring), on the same `reqwest` 0.13 as the rest of
  core. Same site only, `robots.txt` respected, Scrappy's own user agent; depth, pages, files, delay and a smallest
  size are fields of `CrawlRequest`, kept in range in core.
- Each page's file links come from `crawl/links.rs` (`lol_html`): `img` (with lazy `data-src` and `srcset`),
  `source`, `video` and its poster, `audio`, anchors to files, `og:image` and inline `url()`. `grab`'s classifier keeps
  the wanted kinds.
- Files are saved four at a time through `media` (`Downloads::save_now`) into `Images/<domain>/`, inside the crawl's
  one job: it reports files saved of the limit, with the pages visited as its message. Links the catalog already has
  are skipped; a file under the smallest size isn't kept.

## Implementation notes (M12)

- One crawler, three modes (`CrawlMode`): files, mirror and page. `crawl/request.rs` checks the request and starts
  the job; `files.rs`, `mirror.rs` and `page.rs` each run one mode.
- A mirror saves each page the crawler visits into `Sites/<domain>/`, and fetches the stylesheets, scripts, icons,
  images and media it uses (four at a time, within the file limit), plus what the stylesheets use (`url()`,
  `@import`). `crawl/local.rs` maps each address to one path: `/a/b` → `a/b/index.html`, other hosts under
  `_hosts/<host>/`, a query as a short hash in the name. Encoded `..` can't leave the folder.
- When the crawl ends, `crawl/rewrite.rs` (`lol_html`) points every saved page's and stylesheet's links at the copies
  by relative path; links to anything not saved stay absolute. `<base>` and the `integrity` of rewritten links are
  dropped, and the page's declared charset becomes UTF-8 (pages are saved as UTF-8).
- "Save page" is `monolith` 2.12 (default features off) in a blocking task, into `Sites/<domain>/_pages/<path>.html`.
- A mirror or saved page is one `site` row in the catalog (its start page and size), so Home and the Gallery count
  it; the Scrape view opens it in the browser when it's done.

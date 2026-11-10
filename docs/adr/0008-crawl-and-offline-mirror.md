# ADR-0008: Crawl and offline mirror

**Status:** Accepted
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

# ADR-0025: Image search engines as Hunt sources

**Status:** Accepted
**Date:** 2026-11-12
**Amends:** ADR-0013 (Hunt read only web pages, found through a web search) and ADR-0024 (every source was a site)

## Context

Hunt finds web pages about a topic and takes the images in their content (ADR-0013). Image search engines aren't
sites with pages to search that way, so Google Images or Yandex Images couldn't be sources. Nari wants DuckDuckGo,
Bing and Yandex image search in Hunt. Google was left out: its results are built by scripts, it blocks automated
requests often, and its terms forbid it.

## Decision

- **An image search engine is a kind of source,** beside the pack's sites: DuckDuckGo Images, Bing Images and
  Yandex Images. They're listed first in Hunt's sources, on by default, whatever the pack. They're not part of
  packs.
- **Engines give images directly.** Each result is a full-size image and the page it's on; the image is saved
  through `media` with that page as Referer. No page is read and the content finder doesn't run, so the image kind
  switches don't apply to engines; the smallest size and the per-source limit do.
- **Each engine has one reader** in `hunt/engines.rs`, reading what the engine's own search page uses:
  - DuckDuckGo: the `vqd` token from its search page, then its image results as JSON (`i.js`), page by page.
  - Bing: its image results page (`images/async`), each result's `m` attribute (JSON with `murl` and `purl`).
  - Yandex: its image search page, the JSON in its `data-state` attributes; any object with `origUrl` is a result.
- **Yandex reads one page of results** (about 30 images). It answers every later page with a CAPTCHA, cookies or
  not. A CAPTCHA fails that source with a plain message.
- **Each engine host takes turns:** DuckDuckGo's web and image searches share one turn, Bing and Yandex have their
  own, so engines search at the same time as each other but never send one host a burst.
- **Progress** is the same row as a site's: for an engine, it counts results tried of results found instead of
  pages.

## Options considered

- **Google Images:** the most results, but the least reliable to read and against its terms. Left out.
- **Engines inside packs:** a pack would say which engines it uses. Engines fit every topic, so they're picked in
  Hunt once.
- **Reading each result's page instead of taking its image:** slower, and the engine already picked the image.

## Consequences

- **Easier:** many full-size images quickly, from all over the web, without reading pages.
- **Harder:** three more readers that break when an engine changes its page; each one is a function in
  `hunt/engines.rs` with a test on a saved sample of its markup. Images come from any site, with less control over
  what's on them.
- **Rules:** `.claude/rules/architecture/core.md`.

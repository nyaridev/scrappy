# ADR-0013: Hunt: web search and one content-image finder

**Status:** Accepted
**Date:** 2026-11-10
**Supersedes:** ADR-0009's sources and query detection (its duplicate removal stays)

## Context

ADR-0009 planned Hunt around per-source readers (MediaWiki, JSON APIs, search-engine image scrapes) and Wikidata
query detection. Each reader knows one site's API, breaks when that API changes, and finds only what those few sites
host. Nari wants Hunt to work like a person browsing instead: find pages about the topic, and take the images that
are part of each page's content, on almost any website, with a switch for each kind of image data.

## Decision

- **Pages come from a web search:** DuckDuckGo's plain HTML results for the topic, a page of results at a time,
  with a delay between requests. Ads and DuckDuckGo's own links are skipped. This scrapes DuckDuckGo, against its
  terms; it's the one place Hunt depends on a site's markup.
- **One content-image finder for every page** (`hunt/content.rs`). It parses the page into a tree, drops what is
  never content (scripts, navigation, headers, footers, sidebars, forms, and blocks whose class, id or role says
  menu, share, comment, ad, cookie…), scores the remaining blocks by their paragraphs' text and their images, and
  takes the best block as the content. Images in it are kept unless they look like chrome: named or classed as a
  logo, icon, avatar, emoji, sprite or spinner, or declared smaller than 100 pixels. A page with no clear content
  block falls back to the whole cleaned page.
- **A switch per kind of image data,** all read by the same finder:
  - `img`: plain `<img src>`;
  - `responsive`: lazy and high-resolution versions (`srcset`, `data-src` and its cousins, `<picture>` sources),
    the largest candidate taken;
  - `linked`: links around an image that point at an image file (usually the full size);
  - `background`: CSS `background-image` in the content's style attributes;
  - `preview`: the page's `og:image` and `twitter:image`;
  - `inline`: `data:` images and inline `<svg>`, saved as files.
- **Source packs stay, as lists of sites.** A pack (TOML, built-in in `packs/`, the user's own in the config
  folder's `packs/`) has a name, whether it also searches the whole web, and sites; each site is one more search,
  narrowed with `site:`. Every site's results go through the same finder: a pack never adds site-specific code. The
  user picks the pack in the Hunt view; there's no query detection.
- **Saving:** images go through `media` into `Images/<topic>/`, skipping links the catalog already has, with a
  smallest size; the whole hunt is one job. When it ends, the Hunt view offers a duplicate scan of that folder
  (ADR-0009's duplicate removal).

## Options considered

- **Per-site API readers (ADR-0009):** precise where they work, but each one is code to keep alive, and they cover
  few sites.
- **Pasted links instead of a search:** no scraping of a search engine, but no way to start from just a topic.
- **A headless browser for every page:** finds images that scripts add, but is slow and heavy for dozens of pages.
  Pages are read as plain HTTP; Grab's deep scan stays the tool for script-built pages.

## Consequences

- **Easier:** any site works without new code; a pack is a few lines of TOML anyone can write.
- **Harder:** the finder is heuristic and will misjudge some layouts; DuckDuckGo may change its page or refuse
  requests, and then Hunt needs a fix in one place (`hunt/search.rs`).
- **Later:** the finder can serve Grab or the crawl if they need content-only images; another search engine is one
  more module beside `search.rs`.
- **Rules:** `.claude/rules/architecture/core.md`.

## Implementation notes (M13)

- `hunt/search.rs` reads `html.duckduckgo.com/html/` with Scrappy's own user agent: result links come from
  `a.result__a` (DuckDuckGo's `/l/?uddg=` redirect unwrapped, its ad links dropped), and the next page is the form
  furthest on, posted back. 1.5 seconds between requests.
- `hunt/content.rs` parses with html5ever (already in the build through `monolith`) with scripting off, so images in
  `<noscript>` count. A paragraph gives 1 point plus its commas plus up to 3 for length; a content-sized picture
  gives 3, so galleries win without text. Points go to the five blocks around it, less the further out, and a block's
  score shrinks with its share of link text. A best score under 20 means no clear content: the whole cleaned page is
  used. A block named like chrome still counts when it holds half the page's text.
- With "Lazy and high-res" on, a picture's largest version replaces its `src`; off, the plain `src` is kept.
- Pages are read up to 5 MB; the finder runs in a blocking task. Web images are saved through `media` (four at a
  time, the page as Referer); inline ones are written by `hunt` as `inline-<hash>.<ext>`, so the same inline picture
  is saved once.
- The built-in packs are `general`, `games`, `art` and `people`; a user's pack file with the same name replaces one.
- The duplicate panel moved to `src/components/` so Hunt can scan its own folder with it.

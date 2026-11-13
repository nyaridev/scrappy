# ADR-0026: Hunt finds a site's images through image search

**Status:** Accepted
**Date:** 2026-11-12
**Supersedes:** ADR-0013's "pages come from a web search" for a pack's sites (the web search stays for "Any site")

## Context

Each site source found its pages with DuckDuckGo's plain web search (`site:reddit.com <topic>`) and read them with the
content finder. DuckDuckGo started answering those requests with a bot check, so every site source failed with
"DuckDuckGo didn't answer", and nothing in that message said why reddit or Wikipedia depend on DuckDuckGo. Bing's web
search was no way out: it ignores `site:` for these requests. Image searches still answer, and they keep to a site.

## Decision

- **A site source asks image search for that site's images,** trying in turn until one gives results on the site:
  1. Bing Images, `site:<site> <topic>`;
  2. Bing Images, `<topic> <site>` (Bing answers `site:wikipedia.org` with nothing);
  3. DuckDuckGo Images, `site:<site> <topic>`.
  Only results whose page is on the site (or a subdomain of it) count.
- **It saves those images directly, then reads their pages** with the content finder (up to "Pages per source"), so a
  page's other content images come too. The image kind switches apply to the pages.
- **A site source fails only when no engine answered,** with "Bing and DuckDuckGo didn't answer. Try again in a
  while." When they answered with nothing on the site, it ends with "No results".
- **"Any site" keeps DuckDuckGo's web search** (ADR-0013); it's off in the built-in packs.
- Each try takes its engine's turn (ADR-0025), so a site source waits on one host at a time.

## Options considered

- **Bing's web search for sites:** answers, but ignores `site:` for these requests; most results are elsewhere.
- **Retrying DuckDuckGo's web search later:** its bot check didn't clear, and Hunt can't solve it.
- **Only saving the image search results:** fast, but misses the rest of each page's content.

## Consequences

- **Easier:** site sources work while DuckDuckGo's web search refuses; sites built by scripts (reddit, imgur) give
  images even when their pages read as empty.
- **Harder:** a site's images are what Bing and DuckDuckGo indexed for it, not what a search of its pages finds.
- **Rules:** `.claude/rules/architecture/core.md`.

## Implementation notes (packs, 2026-11-12)

- A pack's sites are the ones these searches serve: each site was tried with a topic that suits it, and kept when
  image search found results on it and a hunt saved images from them. Nearly every site answers the second try
  (`<topic> <site>`), not `site:`.
- Left out after trying: imgur.com (almost nothing on the site), wallpaperflare.com, moviestillsdb.com and
  themoviedb.org (nothing or one result), fanart.tv (found, but every download refused), and store.steampowered.com,
  impawards.com, burst.shopify.com, cara.app, gamebanana.com and nationalgeographic.com (a few results at most).
- Some sites only suit their own topics: wikiart.org and artvee.com have no modern art, houzz.com only interiors.
- The 14 built-in packs sit under four headings, a pack file's optional `group`: Photos (General, Wallpapers, Free
  stock photos), Entertainment (People, Movies and TV, Games, Anime and manga, Memes and GIFs), Art and design (Art
  styles, Museum art, Architecture and interiors) and Science and history (Nature and animals, Space, History and
  archives). A user's copy of a built-in pack keeps its heading; a user's own pack without one is under "Your packs".
- The anime pack keeps to boards that hold safe content (zerochan, safebooru); boards that mix in adult art are out.

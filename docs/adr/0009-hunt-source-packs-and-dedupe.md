# ADR-0009: Hunt: source packs, query detection and duplicate removal

**Status:** Accepted
**Date:** 2026-11-10

## Context

Hunt takes a topic ("metin2", "y2k art", a person's name) and collects images from many sources within limits, then
removes duplicates and keeps the best copy. For a person it should find their Instagram and X and offer each as a
checkbox. Different topics need different sources, and the user must be able to add their own. In 2026 Google and Bing
have no usable image API, Brave Search has no free tier, and Getty is hostile to scraping.

## Decision

- **Source packs** are TOML files: built-in ones in `packs/` (games, people, art styles, general), the user's own in
  the app config folder, edited in Settings. Each pack lists sources and their limits. A source is one of a few kinds:
  - a MediaWiki API (Fandom wikis, Wikimedia Commons);
  - a JSON API (Openverse, Danbooru, ArtStation);
  - a search-engine scrape (DuckDuckGo, Bing, Yandex);
  - a profile handed to gallery-dl.
- **Search-engine scrapes are opt-in,** rate-limited and off in the built-in packs: they break those sites' terms and
  break often. Getty isn't included.
- **Query detection** uses Wikidata: the type of the best match (person, video game, art movement…) picks the pack.
  For a person, Wikidata's Instagram, X and YouTube properties become checkboxes. Fandom wikis are found by name.
- **Duplicates:** 64-bit perceptual hashes (`image_hasher`, cached in the catalog) compared by Hamming distance, with
  strict, normal and loose thresholds and an aspect-ratio pre-filter. The best copy wins by pixel count, then format,
  then estimated JPEG quality, then size. The others go to the system trash (`trash` crate); nothing is deleted
  outright.

## Options considered

- **An AI model to classify queries and rank images:** heavier, and needs a key or a local model; Wikidata answers the
  question for named things.
- **Official APIs only:** reliable but thin for art styles and games.

## Consequences

- **Easier:** adding a site is a TOML entry, and packs can be shared as files.
- **Harder:** scraped engines will break and need fixes, and the thresholds need tuning on real sets.
- **Later:** a new source kind is one module in `hunt/sources/`; video dedupe can reuse the hashing on frames.
- **Rules:** `.claude/rules/architecture/core.md`.

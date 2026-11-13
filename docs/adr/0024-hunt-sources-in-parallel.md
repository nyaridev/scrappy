# ADR-0024: Hunt lists its sources and runs them in parallel

**Status:** Accepted
**Date:** 2026-11-12
**Amends:** ADR-0013 (searches ran one after another, with one image limit for the whole hunt) and ADR-0020 (the
built-in packs' "whole web")

## Context

A hunt searched each place in turn: the whole web, then each picked site. Hunt showed one line of progress for all of
it, and one image limit for the whole hunt. The built-in packs leaned on "the whole web", which doesn't say where
the images come from. Nari wants Hunt to list the places it will search, read several of them at once so it isn't
slow, show each one's progress, and limit images per source instead of in total.

## Decision

- **A hunt's sources are listed.** Each picked site is a source. The built-in packs name their sites; General gets
  a list of general image sites instead of the whole web. Web search stays a pack switch, shown as the source
  "Any site (web search)", off in the built-in packs.
- **Sources run in parallel:** four at once. Each source reads two result pages at once, and each page saves its
  images a few at a time, as before.
- **DuckDuckGo gets one search at a time.** Sources take turns for their search, with the usual pause after each,
  because parallel queries get refused. A source waiting for its turn shows "Waiting"; others keep reading pages
  meanwhile.
- **Limits are per source:** "Images per source" and "Pages per source". The hunt's total is the images limit times
  the number of sources. A source stops at its limit, the end of its results, or Stop.
- **Each source's progress is kept in core** (`hunt/status.rs`): its state (waiting, searching, reading, done,
  failed, stopped), its result pages read of found, its images saved of its limit, and its problem if it failed.
  The window reads the latest hunt's sources when job progress arrives, as Grab reads its scan (ADR-0019). The
  job's own progress (images saved of the total) still feeds the Downloads list.
- A source that fails doesn't stop the others. The hunt fails only when nothing was saved and a source failed; it
  then says why.

## Options considered

- **Every source at once:** with up to 30 sites, that's hundreds of requests at the same time. Four sources at once
  keep it fast without flooding the network.
- **Parallel searches too:** DuckDuckGo refuses bursts. One search at a time costs a few seconds per source, while
  the reading, which takes most of the time, runs in parallel.
- **A total limit and a per-source one:** two numbers for one idea. A per-source limit keeps one busy site from
  using up the hunt.

## Consequences

- **Easier:** the user sees where images come from and which source is slow, failing or done.
- **Harder:** more requests at once can trip a site's rate limit sooner. A slow or failing site no longer holds up
  the rest.
- **Rules:** `.claude/rules/architecture/core.md`.

## Implementation notes

- `hunt/run.rs` runs the sources with `buffer_unordered(4)` and the pages with `for_each_concurrent(2)`. A shared
  `tokio::sync::Mutex` is the search turn, held through the search and its pause. Pages and images already seen are
  shared, so two sources never save the same image.
- An image takes a slot from its source's count before it downloads and gives it back if it isn't kept, so a source
  never goes over its limit, even with pages read at once.
- `Hunts` (managed by the shell) keeps the latest hunt's sources. A ticker reports job progress every 500 ms while
  the hunt runs, so the window sees state changes even when no image is saved.

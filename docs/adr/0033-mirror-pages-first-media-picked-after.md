# ADR-0033: A mirror saves its pages first; its images and videos are picked after

**Status:** Accepted
**Date:** 2026-11-12
**Amends:** ADR-0008 (a mirror saved every asset of its pages, up to a file limit)

## Context

Mirror site saved each page with everything it uses, images and videos included, up to a number of files (200 by
default) and 50 MB a file. On a big site the limit runs out on the first pages' pictures, and a site with clips fills
the disk with videos nobody wanted. An earlier archive of a fan site (36,000 pages, 20,000 files) worked better in two
passes: every page first, then the files they use, with videos left as links to the live site unless asked for, and
nothing fetched twice when it ran again. Nari wants a mirror to work that way, with the images and videos shown as
Grab shows its finds, to pick from.

## Decision

- **Crawl:** a mirror saves every page within its depth and pages, with what makes it look right: stylesheets (and
  everything they use), scripts, fonts and icons. There's no file limit. The images and videos its pages show
  (`img`, `srcset`, `video`, `source`, `poster`, inline `url()`, and links to image and video files) are listed, not
  fetched.
- **Links:** each page's links to saved files point at the copies; every other link (a page past the limits, an
  image not saved yet) points at the live site, so the mirror still shows it online.
- **Pick:** Scrape lists the mirror's images and videos as Grab does (grid or list, thumbnails, type filter, sort,
  picking), growing while it crawls. Saving the picks streams each file into the mirror's folder, skips a video over
  **Max video size** (shown as Too big), and then points the pages at the new copies. Each saving gets a row in the
  Download list.
- **Record:** each mirror's pages, stylesheets, saved files and listed media, and how saving each went, are kept in
  `.scrappy/mirrors/<site>.json` in the library, so media can be picked later, even after a restart. A file already in
  the mirror isn't fetched again. A new mirror of the same site replaces the record.
- **Grab's results** (the toolbar, grid, list and paging) move to `src/components/finds/`, shared by Grab and Scrape.

## Options considered

- **Keep tray and Max video size before the crawl:** decides before seeing what the site has.
- **Fetch sizes during the crawl (a HEAD per image):** thousands of extra requests to the site; sizes show as unknown
  until a file is saved instead.

## Consequences

- **Easier:** a mirror of any size finishes its pages; only the wanted media is downloaded, and more can be picked
  later.
- **Harder:** a mirror needs a second step for its pictures; until then they load from the live site. The window
  polls the media list once a second while a crawl or a save runs (shortcut: the whole list each time; send changes
  only if mirrors of tens of thousands of files feel slow).
- **Rules:** `.claude/rules/architecture/core.md`, `.claude/rules/architecture/frontend.md`.

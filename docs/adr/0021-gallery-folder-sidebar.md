# ADR-0021: The Gallery's folder sidebar

**Status:** Accepted
**Date:** 2026-11-12
**Amends:** ADR-0015's filters (the site filter is replaced; the type filter, search and sort stay)

## Context

The Gallery filters by type and by site, from a menu. Sites are a catalog fact some files don't have (a file added by
hand, a mirror), and the library is already sorted into folders by kind, site and account (`Videos/NASA`,
`Social/twitter/<user>`). Nari wants a sidebar of folders to narrow the Gallery down, in place of "All sites", and the
search to run as soon as a key is typed.

## Decision

- **A folder sidebar** beside the grid: "All files", then the library's folders as a tree, each with how many files
  it holds (its subfolders' too). A click on a folder shows the files in it and below it, and opens it; only its
  arrow, a button of its own, closes it. iro-ui's tree row is one button that only toggles, so the rows are the app's
  own (`FolderRow`). Right-click opens a folder in the file manager. The type filter, search and sort (plain
  dropdowns, not typed into) narrow it further.
- **Core sends the folders** with each page: every folder that holds a file, by path, with its count, in name order.
  The query takes a `folder` instead of a `site`.
- **The search runs on each key.** A page is a read of the folder listing core already keeps (ADR-0015), and an
  older answer that arrives late is dropped, so no pause is needed before asking.

## Options considered

- **iro-ui's `FileTree` or `TreeRow`:** a click on a folder only opens or closes it; folders here are what's picked.
- **Keeping the site menu beside the sidebar:** two ways to say nearly the same thing; the site still shows on each
  tile.

## Consequences

- The site filter is gone; `GalleryPage.sites` becomes `folders`.
- A library of thousands of account folders makes a long sidebar; it scrolls on its own.

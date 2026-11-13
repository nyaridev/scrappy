# ADR-0022: Deleting files and folders from the Gallery

**Status:** Accepted
**Date:** 2026-11-12
**Amends:** ADR-0015 (the Gallery could only show and open files) and ADR-0021 (the folder sidebar)

## Context

Removing something downloaded means leaving the app for the file manager. Nari wants a right-click Delete on a
Gallery tile and on a downloaded folder in the sidebar, deleting it for good, after a confirmation.

## Decision

- **Delete** is in the right-click menu of every Gallery tile and of every folder in the sidebar below the top
  level. The library's own folders (`Videos`, `Audio`, `Social`, `Grabs`, `Images`, `Sites`) have no Delete, and
  nothing in `.scrappy` (the catalog, thumbnails) can be deleted.
- **A confirmation** names what goes and says it's deleted from the disk, not moved to the trash. Only its Delete
  button deletes.
- **It's permanent.** Core deletes the file, or the folder with everything in it, and drops their rows from the
  catalog. It checks the path in Rust first: it must be inside the library (`library::locate`), a folder must be
  below the top level, and nothing may be in `.scrappy`. The Gallery reads the folder again through the library
  watcher (ADR-0015).

## Options considered

- **The system trash,** as the duplicate finder does (ADR-0009): Nari asked for deletion for good; the confirmation
  stands in for the trash's undo.
- **Deleting the library's own folders too:** setup made them and downloads expect them.

## Consequences

- There's no undo. Download list rows that point at a deleted file stay, as they do when a file is deleted outside
  the app.
- Thumbnails made for deleted files stay in `.scrappy/thumbs/` until the library is cleared by hand.

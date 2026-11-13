# ADR-0039: Changing the library folder

**Status:** Accepted
**Date:** 2026-11-13
**Amends:** ADR-0004 (the library folder can change after first-run setup)

## Context

The library folder is picked once, at first-run setup, and can't be changed after: a full drive or a new disk leaves
Scrappy stuck on the old folder. Two different wishes hide behind "change the folder": use another folder from now on
(a new one, or a library made before), or take this library, files and catalog, to a new place.

## Decision

- **Settings → Library has both:** "Use another folder" and "Move library".
- **Use another folder** goes through first-run setup's own check and `library::setup`: a new or empty folder gets
  the layout and a catalog, a Scrappy library keeps its catalog, a folder with other files gets the layout beside them
  (nothing in it is moved or deleted). The settings switch at once; the old folder and its files stay where they are.
- **Move library** is a job (`library::start_move`, ADR-0010): it copies every file of the library, the catalog and
  `.scrappy` with them, into a new or empty folder outside the library, checks the copy (every file there, at its
  size), then switches the settings to it. The old folder stays as it was: Scrappy never deletes the user's files,
  and the user removes it when they're sure.
- **Stopped or failed,** a move removes only what it copied, leaving the new folder as it found it, and the settings
  keep the old library.
- **Refused while downloads run:** they write into the library and its catalog, so a copy made meanwhile could miss
  them. A download started during a move goes to the old folder.
- After either, the shell sends `library-changed` (ADR-0015), so the Gallery lists the new folder; the other views load
  again when they show.

## Options considered

### A: Switch only
- **Pros:** almost no new code.
- **Cons:** the library's files and history stay behind; moving them by hand loses nothing only if the catalog moves
  too.

### B: Rename the folder (a real move)
- **Pros:** instant on the same drive.
- **Cons:** fails across drives, which is the usual reason to move; a half-done move across drives leaves files in two
  places.

### C: Copy, check, switch (chosen)
- **Pros:** works across drives; the old library is untouched until the user removes it, so a failed move loses
  nothing.
- **Cons:** needs the space twice for a while, and takes as long as copying the library.

## Consequences

- **Easier:** a new drive or a full one is a Settings change, not a reinstall.
- **Harder:** the user removes the old folder themselves after a move.
- **Rules:** `core.md` (`library` owns the move).

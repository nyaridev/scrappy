# ADR-0020: Editing Hunt packs in the app

**Status:** Accepted (new, removed and hidden packs: ADR-0028)
**Date:** 2026-11-12
**Amends:** ADR-0013's source packs (still TOML files of sites; now the app writes them too)

## Context

Packs are TOML files: built-in ones in `packs/`, the user's own in the config folder's `packs/`. Changing one means
opening that folder and editing a file by hand. Nari wants an Edit button in Hunt to add, remove and switch off a
pack's sites.

## Decision

- **Edit** beside the Pack menu opens a dialog for the picked pack: its name, whether it searches the whole web, and
  its sites, each with an on/off switch and a remove button, and a field to add one. A link pasted there is kept as
  its site name (`https://www.artstation.com/x` → `artstation.com`, with `www.` dropped).
- **Saving writes the user's pack file**, `<config>/packs/<id>.toml`, so an edited built-in pack is replaced by the
  user's copy the way ADR-0013 already reads them. Core checks it as it checks a file (a name, at most 30 sites, each
  a plain host, some place to search) and writes it whole (a temporary file, then renamed). Comments in a hand-edited
  file aren't kept.
- **A site switched off** stays in the pack, listed in its new `off` list: Hunt shows it unticked, and it can still be
  ticked for one hunt. A pack file without `off` has every site on.

## Options considered

- **A pack editor in Settings** (ADR-0009's first plan): the sites are picked in Hunt, so they're edited there.
- **Removing instead of switching off:** a site the user only wants to skip for now would have to be typed again.

## Consequences

- An edited built-in pack no longer follows changes to the built-in one; removing the user's file brings it back.
- New packs and removing a user's pack aren't in the window yet: they're files in the packs folder, as before.

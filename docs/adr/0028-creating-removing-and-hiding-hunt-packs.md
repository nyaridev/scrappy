# ADR-0028: Creating, removing and hiding Hunt packs

**Status:** Accepted
**Date:** 2026-11-12
**Amends:** ADR-0020 (packs are edited in the app; new and removed packs were left to the packs folder)

## Context

Hunt's packs can be edited in the app (ADR-0020), but a new pack or removing one still means working in the packs
folder by hand. With 14 built-in packs (ADR-0026's notes), the Pack list is also long for someone who uses a few.
Nari wants to create and remove their own packs in the app, to keep the built-in ones from being removed or renamed,
and to hide any pack from the Pack list.

## Decision

- **New pack:** the packs dialog makes a pack with a name and sites, saved as the user's `<id>.toml` in the packs
  folder. Core picks the id from the name (`My cats!` → `my-cats`) and never reuses one that's taken, so a new pack
  can't replace a built-in one. It's listed under "Your packs" (ADR-0026's notes).
- **Removing** deletes the user's pack file, after the window asks. Core refuses a built-in pack's id.
- **Built-in packs keep their name:** core lists them with a `builtIn` flag; the editor shows the name but can't change
  it, and a saved copy (ADR-0020) keeps the built-in name, also when the file says another.
- **Hidden packs** are a setting, `hiddenPacks` (pack ids), not a pack file field: hiding a built-in pack doesn't need
  a copy of it, which would stop it following updates (ADR-0020). Hidden packs are left out of the Pack list; the
  dialog's "Show in the Pack list" switch brings one back. The last shown pack can't be hidden.
- **All of it is in one dialog,** ADR-0020's Edit beside the Pack list: a picker of every pack (under its heading,
  hidden ones marked) and New pack at the top, then the picked pack's name, its switch to show it, the web search and
  its sites, and Delete in the footer for the user's own. It opens on the pack Hunt has picked.

## Options considered

- **A `hidden` field in the pack file:** a built-in pack would need the user's copy just to hide it.
- **The packs in Settings:** packs are picked and edited in Hunt, so they're managed there.
- **A separate manager dialog** (a list with a switch, Edit and Delete per pack): tried first, then folded into the
  Edit dialog, so packs have one place.
- **Letting a new pack take any id the user types:** an id is a file name; a typed one could clash with a built-in.

## Consequences

- **Easier:** packs for the user's own topics without touching files; a shorter Pack list.
- **Harder:** a removed pack is gone (its file is deleted, not moved to the trash); the window asks first.
- **Later:** a "Reset" for an edited built-in pack would be the same removal of the user's copy, allowed for built-ins.
- **Rules:** `.claude/rules/architecture/core.md`, `frontend.md`.

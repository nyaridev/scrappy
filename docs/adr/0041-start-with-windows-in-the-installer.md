# ADR-0041: Start with Windows in the installer

**Status:** Accepted
**Date:** 2026-11-13
**Amends:** ADR-0036 (the installer offers the choice; the app still owns the start entry)

## Context

Starting with Windows is off until the user finds the switch in Settings. Nari wants the installer to ask, checked by
default, as installers usually do. ADR-0036 turned down an installer box because the system's entry and the
preference would drift apart: the installer writes one, the app doesn't know.

## Decision

- **The Windows installer's Welcome page has "Start Scrappy with Windows", checked.** Tauri's installer takes hooks
  (`src-tauri/windows/hooks.nsh`, set in `tauri.release.conf.json`); they're read before its pages, so the box goes on
  the first page, the Welcome page.
- **The installer only hands the choice over:** it writes `StartWithSystem` (1 or 0) under
  `HKCU\Software\com.nyaridev.scrappy`. It doesn't touch the start entry itself.
- **The app takes it once:** at start, a release build reads the value, saves it as the `startWithSystem` preference,
  turns the start entry on or off to match (ADR-0036), and deletes the value. From then on Settings owns it as before.
- **Updates leave it alone:** an update installs in passive mode, which skips the Welcome page, so nothing is handed
  over; a silent install (`/S`) is the same.
- **Uninstalling** (not updating) removes the start entry, so Windows doesn't try to start a Scrappy that's gone.
- macOS and Linux have no installer page for it; there, Settings is the only way, as before.

## Options considered

### A: The installer writes the start entry itself
- **Pros:** works before the app ever runs.
- **Cons:** Settings shows the switch off while Scrappy starts with Windows; the app's next save undoes it.

### B: Turn it on by default in the app
- **Pros:** no installer change.
- **Cons:** nobody is asked; it's on for builds that weren't installed.

### C: The installer hands the choice to the app (chosen)
- **Pros:** one owner for the entry and one switch that's always right; the user is asked where they expect it.
- **Cons:** the choice applies at the first start after installing, not before (the installer's "Run Scrappy" box
  starts it at once).

## Consequences

- **Easier:** Scrappy starts with Windows from the first sign-in after installing, unless the box was unchecked.
- **Harder:** the hooks depend on Tauri's installer template: its Welcome page and its passive mode. A Tauri update
  that changes them needs a look at `hooks.nsh`.
- **Rules:** `shell.md` (`autostart.rs`, `windows/hooks.nsh`).

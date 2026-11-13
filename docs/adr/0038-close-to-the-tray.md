# ADR-0038: Close to the tray

**Status:** Accepted
**Date:** 2026-11-13
**Amends:** ADR-0036 (started by the system, Scrappy waits in the tray, not on the taskbar)

## Context

The in-page buttons (ADR-0035) and the extension need the app running, and downloads stop when it quits. Closing the
window quits Scrappy, so a download or the extension is one click from stopping. Nari wants closing the window to
keep Scrappy running in the tray, on by default.

## Decision

- **A tray icon while Scrappy runs** (the notification area on Windows, the menu bar on macOS, an indicator on
  Linux), from Tauri's own tray (its `tray-icon` feature), not a plugin. A click shows the window; its menu has "Open
  Scrappy" and "Quit Scrappy".
- **"Close to tray" in Settings** (About), saved in the preferences (ADR-0034) as `closeToTray`, on by default.
  While it's on, closing the window (its close button, the title bar's, Alt+F4) hides it; Scrappy keeps running,
  downloads and all. Off, closing quits, as before.
- **Quit Scrappy** in the tray's menu always quits.
- **Started by the system** (ADR-0036) with "Close to tray" on, the window starts hidden in the tray; off, it starts
  minimized, as before.
- **A second start** brings the hidden window back, as it brings a minimized one forward.
- The shell owns it (`src-tauri/src/tray.rs`): the tray and the close are Tauri's. It reads the preference from
  core's settings each time the window is closed, so a change applies at once. The window gets no new permissions
  (ADR-0005).

## Options considered

### A: Minimize instead of close
- **Pros:** no tray icon.
- **Cons:** the window stays on the taskbar; not what closing to the tray means.

### B: Tauri's tray, a close that hides (chosen)
- **Pros:** built into Tauri; one small shell file; the window and its state are kept, so showing it is instant.
- **Cons:** the hidden window's web view keeps its memory; Linux needs an appindicator (the build already installs
  it).

### C: Destroy the window and make a new one on show
- **Pros:** frees the web view's memory.
- **Cons:** views lose their state (they stay mounted to keep it), and showing it again takes a moment.

## Consequences

- **Easier:** downloads and the extension keep working after the window is closed.
- **Harder:** quitting is in the tray's menu; someone who expects the close button to quit turns the switch off.
- **Rules:** `shell.md` (`tray.rs`).

# ADR-0011: Frameless window

**Status:** Accepted
**Date:** 2026-11-10

## Context

Nari wants Scrappy's window frameless, like ume-sync and PrepKit: the header is the title bar, with no system frame
above it. It must work on Windows, macOS and Linux, and fall back to the system frame when the custom one misbehaves
on some setup (a Linux window manager, a remote desktop).

## Decision

- **Rust builds the window** (`src-tauri/src/window.rs`), not `tauri.conf.json`, so it gets its frame before it first
  shows: no flicker from a frame being removed after start.
- **Core picks the frame** (`scrappy_core::app::window_frame()`), and `AppInfo.frame` tells the header what to draw:
  - `custom` on Windows and Linux: no system frame. The header is the drag area (double-click maximizes), and
    minimize, maximize/restore and close sit at its right. The system shadow and resize borders stay (no
    `transparent` window: it costs speed and breaks on some Linux setups).
  - `overlay` on macOS: the system's own traffic lights over the header's left end (`TitleBarStyle::Overlay`, hidden
    title), the header is the drag area, and Scrappy draws no buttons.
  - `native`: the system's own title bar over a plain header. Picked on every OS when Settings → "Use the system
    title bar" is on. Rust builds the window once, at start, so the switch applies the next time Scrappy opens.
- **The window gets the window-control permissions** it needs, on top of `core:default`: `start-dragging`,
  `internal-toggle-maximize` (double-click on the drag area), `minimize`, `toggle-maximize` and `close`. They move
  and close the window only; the window still has no fs, shell, http or opener reach (ADR-0005).

## Options considered

- **Frameless from `tauri.conf.json`:** simplest, but the native fallback would have to put the frame back after
  start, and macOS would need its own config file.
- **Custom buttons on macOS too:** rejected. Mac users expect the traffic lights at the left, and the native ones keep
  full-screen and window-snapping behavior.
- **`tauri-plugin-decorum`:** adds Windows 11 snap layouts on the maximize button, but it's one more plugin for one
  hover menu.
- **An environment variable for the fallback:** works before settings exist, but nobody finds it. Replaced by the
  Settings switch in M1.

## Consequences

- **Easier:** the header and the window frame always agree, because both follow `AppInfo.frame`.
- **Harder:** hovering maximize doesn't open Windows 11 snap layouts (Win+Z and dragging to the screen edges still
  work).
- **Later:** applying the switch without a restart means rebuilding the window, which isn't worth it for a setting
  changed once.
- **Rules:** `.claude/rules/architecture/shell.md`, `frontend.md`.

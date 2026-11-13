# ADR-0036: Start with the system

**Status:** Accepted (amended by ADR-0038: with "Close to tray" on, it starts hidden in the tray; by ADR-0041: the
Windows installer asks, checked by default, and the app takes the answer)
**Date:** 2026-11-13

## Context

The in-page buttons (ADR-0035) send to the running app; scrappy-host starts it when it's closed, which makes the
first send wait for the app to open. Nari wants Scrappy to start with Windows, so it's ready.

## Decision

- **A "Start with Windows" switch in Settings** ("Open at login" on macOS and Linux), saved in the preferences
  (ADR-0034) as `startWithSystem`, off by default.
- **Tauri's autostart plugin** does it (the per-user Run key on Windows, a LaunchAgent on macOS, an autostart entry on
  Linux). The shell turns it on or off when the preference is saved; core keeps no Tauri code. The window gets no
  plugin permissions (ADR-0005): the shell calls the plugin from Rust.
- **Started by the system, the window opens minimized** (the plugin passes `--minimized`): Scrappy has no tray icon,
  so it waits on the taskbar.
- **At start, a release build turns it back on** when the preference says so and the system lost it (an upgrade's
  uninstall step removes it). It never turns it off at start. Debug builds leave it alone, so a dev run never points
  it at `target/debug`.

## Options considered

- **A tray icon and starting hidden:** a second way into the app to keep working, for one feature.
- **The installer's own "start with Windows" box:** only on Windows, and only at install time.

## Consequences

- **Easier:** the first send after signing in is as fast as any other.
- **Harder:** one more plugin in the shell, and its state lives in two places (the preference and the system); the
  preference wins, at save and at start.

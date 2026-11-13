# ADR-0023: The Gallery's viewer and video player

**Status:** Accepted
**Date:** 2026-11-12
**Amends:** ADR-0005 (the window's CSP gets `media-src` for one app protocol) and ADR-0015 (pictures open in a
lightbox; videos and audio opened in their app)

## Context

A picture opens large, one at a time, and videos and audio open in another app. Nari wants to step through the
Gallery with the arrow keys and play videos in the app, with its own controls in the Sumi style: the volume kept
between runs, playback speed, frame-by-frame steps, and the other things a player is expected to do.

## Decision

- **One viewer** over the Gallery for pictures, videos and audio. ← and → (and buttons at its sides) go to the
  previous or next file the Gallery shows that the viewer can show; other files still open in their app. Its top bar
  names the file and has Open in app, Show in folder and Close. It covers the views, not the header, so the title bar
  still drags the window; switching to another view closes it, and coming back opens it on the same file.
- **Each Gallery file says what the viewer can do with it** (`GalleryItem.view`: picture, video, audio, or none),
  from its extension in core: a picture of a web type up to 30 MB (sent whole, as before), and the video and audio
  types the window's browser plays.
- **Videos and audio stream from Rust** through an app protocol, `scrappy:` (`http://scrappy.localhost/` on
  Windows). Core serves a library file's bytes a range at a time (at most 4 MB per answer), so the player can seek
  and nothing is read whole. It checks the path as every other library command does (`library::locate`) and serves
  only video and audio types. The CSP gets `media-src 'self' scrappy: http://scrappy.localhost`; nothing else
  changes.
- **The player is the app's own** (`<video>` with no browser controls): play and pause, a seek bar with times,
  previous and next frame, ±10 seconds, volume and mute, speed (0.25× to 2×), loop and full screen, with keyboard
  keys for each. The controls hide while a video plays and the pointer rests. A file the browser can't play says so
  and offers Open in app.
- **Volume and mute are settings** (`Settings.player`), saved by Rust like the others, so they're kept between runs.
  The speed is kept while the viewer is open.

## Options considered

- **Tauri's asset protocol:** serves any file in a scope set by path globs; the library folder changes at runtime,
  and checks would live in a config scope rather than in core beside the other library checks.
- **`data:` URLs, as for thumbnails:** a video would travel whole, and couldn't seek before it arrived.
- **A frame rate from FFprobe** for the frame steps: the player measures it while it plays instead
  (`requestVideoFrameCallback`), 30 fps until it knows.

## Consequences

- Codecs the browser lacks (HEVC without the system's extension, some MKVs) don't play in the app; the viewer says
  so and opens them in their app.
- `media-src` lets the window load media from the app protocol only; remote media stays refused.

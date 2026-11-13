# ADR-0043: MP4 and MP3 buttons on YouTube

**Status:** Accepted
**Date:** 2026-11-13
**Amends:** ADR-0035 (YouTube's button), ADR-0006 (`send` names a preset)

## Context

On YouTube, the in-page Download button sends the video with the start preset from Settings. Getting the audio
alone takes a trip to Settings, or to the app's Download view. Nari wants the choice on the page itself.

## Decision

- **YouTube's watch page gets two buttons, MP4 and MP3, instead of Download.** Both sit at the end of the action row
  with YouTube's look. Each is labelled with its format, and its tip reads "Download as MP4 with Scrappy".
- **A send can name a preset.** `send` has an optional `preset`, a preset's id: MP4 sends `editor-mp4` (MP4, H.264)
  and MP3 sends `mp3` (320 kbps). The app looks the id up in its presets and refuses one it doesn't know. With no
  preset, the start preset applies as before, so the other sites, the overlay and the popup don't change.
- **A button's `format` keeps buttons on one anchor apart.** A site's place can carry a `format` (label and preset),
  and the buttons on one anchor are kept by format, so YouTube's two buttons update in place as the page changes.

## Options considered

### A: One button with a menu
- **Pros:** one button's room in the row.
- **Cons:** two clicks for every download, and a menu inside YouTube's row is fragile.

### B: Two buttons (chosen)
- **Pros:** one click for either format.
- **Cons:** the formats are fixed: another one (M4A, FLAC) means another button, or the app.

## Consequences

- **Easier:** audio from YouTube is one click.
- **Harder:** the extension names two preset ids; renaming those presets in the app breaks the buttons.
- **Rules:** `extension.md` (a button's format and preset).

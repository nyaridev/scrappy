# ADR-0035: In-page download buttons

**Status:** Accepted; YouTube embeds and asking before sending a stream: ADR-0042
**Date:** 2026-11-13
**Amends:** ADR-0006 (the extension runs a content script on every page; a sent video uses the start preset)

## Context

The popup takes a click to open and a click to send. On the sites Nari downloads from most (YouTube, SoundCloud, X,
Twitch clips) the video is right there, and a Download button beside the site's own Share or Like button would be one
click. Other sites have no such place, but most show their video in a `<video>` element. Sends from the extension
used the first preset with no quality limit, ignoring the start preset and quality set in Settings (ADR-0034).

## Decision

- **A content script on every page** (`extension/entrypoints/buttons.content.ts`, top frame, at document idle) adds
  Download buttons. It finds where with one small file per site in `extension/lib/sites/`: YouTube (the watch page's
  action row), SoundCloud (a track's action row), X (a post with a video: its action row, sending the post's link)
  and Twitch (a clip's page). Each file only names its places; the shared code adds the buttons and watches the page
  (a throttled `MutationObserver`) for the pages these sites load without reloading.
- **Every other site:** a button shows in the top corner of a `<video>` at least 200 by 120 pixels while the pointer
  is on it. It sends the video's own file when that's an `http(s)` link, else the largest stream the background script
  saw for the tab, else the page's link (yt-dlp knows many sites).
- **The page's look:** each button lives in a closed Shadow DOM, so the page's CSS can't reach it, with its own CSS
  shaped like the site's buttons (a pill on YouTube, a small outlined button on SoundCloud, an icon on X). Colors come
  from the site's light or dark mode. It shows Scrappy's arrow and "Download", then Sending, Queued, or the error.
- **The click goes through the background script,** which sends the same `send` request the popup does
  (`sendToApp`); content scripts can't use native messaging. When the app asks for a sign-in (`needsCookies`), the
  button says to use the popup: cookies are only read there, after the user's choice (ADR-0005).
- **The app downloads it with the Settings start preset and highest quality** (ADR-0034), for popup sends too. A link
  with no video (a SoundCloud track) takes the start preset when it's an audio one, else the first audio preset (MP3).
- **The popup stays** for picking among a page's videos and images, and for "Grab all".

## Options considered

- **Buttons styled with the site's own CSS classes:** they'd match exactly, but break, or pick up the site's click
  handlers, whenever the site changes its markup. A shadow root with lookalike CSS only breaks where it's placed.
- **One generic button on every site:** no per-site code, but on YouTube or X the `<video>` is a `blob:` stream, so
  the page or post link is what's worth sending, and the corner overlay fights the site's own player controls.
- **A setting to turn the buttons off in the extension:** the extension keeps no settings (ADR-0006). Turning the
  extension off, or limiting its sites in the browser, does it.

## Consequences

- **Easier:** one click on the sites used most, and anywhere a page plays a `<video>`.
- **Harder:** a site's markup change can hide its button until its file in `lib/sites/` is updated; the generic
  overlay still works there.
- **Permissions:** no new ones. The content script runs under the `<all_urls>` host permission ADR-0006 already has.
- **Rules:** `extension.md` (how to add a site).

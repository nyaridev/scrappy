# ADR-0042: The extension reads the page first

**Status:** Accepted
**Date:** 2026-11-13
**Amends:** ADR-0005 (a sign-in can be shared without a download), ADR-0006 (new messages), ADR-0034 (the default
file name), ADR-0035 (which link a button sends)

## Context

Testing the extension on real sites showed five problems:

- On a site yt-dlp knows, the popup's top row reads "Page" with a big Download button, as if it saved the web page.
  The video itself isn't shown.
- Instagram, X and others need a sign-in. It can only be shared when a download asks for it.
- The popup's lists can't be narrowed by size, and most images and streams show no size.
- A YouTube video embedded in another page gets no button. The overlay looks only for `<video>` elements, and the
  embed's player sits in a frame from another site.
- On a site yt-dlp knows (Pornhub), the overlay sent the HLS stream the tab loaded instead of the page. yt-dlp's
  generic reader named it `master [master]` and saved it loose in `Videos/`.

## Decision

- **New messages.**
  - `look {url}`: the app reads the page with the engine its route names (yt-dlp, or gallery-dl for posts and
    profiles), with yt-dlp's generic reader turned off (`--ies default,-generic`). A link no reader knows fails at
    once, without a request to the site. The answer is `video` (title, channel, duration, thumbnail, count for a list)
    or `notVideo`, or `needsCookies`. The probe is kept as any other (ADR-0016), so a Download that follows starts at
    once.
  - `site {url}`: the page's site, and whether its sign-in is remembered.
  - `signIn {site, cookies}`: remembers that site's cookies, checked as a grant's are, and downloads nothing.
- **The popup leads with the video.** While `look` runs, a placeholder card shows. A known page gets a card with
  its picture, title, channel and length, and the Download button. Any other page gets one small "Send page" line
  instead of the big row; the Videos and Images lists follow as before.
- **Share sign-in.** The popup has a "Share sign-in" button for the tab's site ("Update sign-in" once one is
  remembered). On a click it reads that one site's cookies and sends `signIn`. That one site's cookies leave the
  extension only on the user's click, which keeps ADR-0005's rule.
- **Size filters.** The Videos and Images lists get a smallest-size filter. A size comes from the response,
  else from the page's resource timing, else it's estimated from the pixels (and the length for a video) and shown
  with `~`. Items with no size and no estimate show only under "Any".
- **Embeds.** The overlay also covers YouTube's embedded players (`/embed/<id>` on youtube.com and
  youtube-nocookie.com), sending the video's watch link. It shows when the pointer enters the frame, since the
  frame's own moves never reach the page.
- **The overlay asks before it sends a stream.** For a `blob:` video, the background script sends `look` for the
  page. A known page goes as the page; any other sends the tab's best stream as before.
- **Streams are named from their page.** A send carries the page and its title. When yt-dlp read the link with its
  generic reader, the file takes the page's title and site instead of the stream's name, and the Download list
  shows that title.
- **Videos are filed by site, then creator.** `{site}` is the registrable domain of the video's page
  (`youtube.com`, `x.com`), or of the page a stream was sent from. The default file name becomes
  `{site}/{channel}/{title} [{id}]`, and a saved setting still at the old default moves to the new one. Playlists go to
  `Videos/{site}/{channel}/{playlist}/`. A field left empty leaves no `[]` or `()` behind.

## Options considered

### A: Keep a list of yt-dlp's sites in the extension
- **Pros:** no round trip.
- **Cons:** about 1800 readers, and the list goes stale with every yt-dlp update.

### B: Ask the app, with yt-dlp's generic reader off (chosen)
- **Pros:** always matches the installed yt-dlp. An unknown page answers in about a second. A known page's read is
  reused by its download.
- **Cons:** a yt-dlp run each time the popup opens. A known page takes a few seconds to show its card.

## Consequences

- **Easier:** the popup shows what will download. Signed-in sites can be set up ahead. Streams land in named folders.
- **Harder:** opening the popup on a known page reads it, which costs a request to that site.
- **Rules:** `extension.md` (the messages, sign-in, embeds), `security.md` (sharing a sign-in), `core.md`.

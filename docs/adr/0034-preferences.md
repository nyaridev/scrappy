# ADR-0034: Preferences: download extras, names, a proxy, starting values

**Status:** Accepted; the default file name files videos by site: ADR-0042
**Date:** 2026-11-12
**Amends:** ADR-0004 (the settings file), ADR-0012 (yt-dlp updates itself at every start), ADR-0029 (removing a
grab's duplicates is always strict)

## Context

Settings held the library folder, how downloads run, the player's sound and the look. Everything else was fixed in
code: what yt-dlp writes besides the video, where a download goes and what it's named, the preset and quality the
Download view starts on, each view's starting limits, how alike pictures must be for Grab and Hunt to remove one, and
that yt-dlp updates itself at every start. The app reached the internet directly, with no way through a proxy. Nari
wants these under control.

## Decision

- **One `Preferences` group in the settings file,** saved as a whole with one command (`set_preferences`) and
  brought into range in core (`Preferences::checked`). Missing fields take their defaults, so older files load.
- **Downloads:** subtitles (on or off, the languages as yt-dlp takes them, `en,pl`, and whether auto-generated ones
  count), embedded into the video; embedding metadata, the thumbnail and chapters into the file; the preset and the
  highest quality the Download view starts on; and the name of a single download inside `Videos/` or `Audio/`, a
  path template (`{channel}/{title} [{id}]` by default) with a preview from core. Playlists keep their folder per
  playlist.
- **Proxy:** one `http`, `https`, `socks5` or `socks5h` address, without a user or password (those would be secrets in
  a plain file, ADR-0005). yt-dlp and gallery-dl get `--proxy`, the app's own requests go through it (a new core
  module, `net`, builds every HTTP client), the mirror's crawler gets it, and Grab's browser gets `--proxy-server`.
- **Starting values:** what Grab (Smallest, Remove duplicates), Scrape (Depth, Pages, Delay, Max video size) and Hunt
  (Pages and Images per source, Smallest, engines) show when they first open.
- **Duplicates:** how alike pictures must be when Grab and Hunt remove copies on their own (Strict by default, as
  before). The duplicate finder keeps its own choice.
- **yt-dlp updates at start:** on by default, as before; off leaves it until the user updates from Tools.
- Core keeps the values the tools read in one place each (`media::engine` for yt-dlp's, `net` for the proxy,
  `dedupe` for strictness), set by `app::apply` at start and after each save, as the speed limit already is.

## Options considered

- **A setter and a command per group:** four more of each for the same save.
- **Remembering each view's last values instead of starting values:** values that change by themselves are harder to
  reason about; the user asked for settings.
- **A proxy password in the vault:** possible later; no one needs it yet.

## Consequences

- **Easier:** downloads come out named and tagged the user's way; the app works behind a proxy.
- **Harder:** a preset that converts after downloading (ffmpeg) may drop embedded subtitles, chapters or the cover;
  a page save (`monolith`) and the thumbnails ffmpeg reads from the web don't use the proxy yet.
- **Rules:** `.claude/rules/architecture/core.md`.

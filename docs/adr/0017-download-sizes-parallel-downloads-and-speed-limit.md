# ADR-0017: Download sizes, parallel downloads and a speed limit

**Status:** Accepted
**Date:** 2026-11-11
**Supersedes:** the download queue's fixed two at once (ADR-0010's limit per kind, for downloads)

## Context

Nari wants to see how big a download is before starting it and how big it was after, to download more at once (a
channel's videos come one after another today, and only two downloads run at a time), and to cap the speed. Speeds
are shown in MB/s, the unit people see on their files, not Mbps.

## Decision

- **Size before:** for a single video, Rust asks yt-dlp to run the preset's format choice again on the probe it kept
  (`--load-info-json`, `--simulate`, printing `filesize` or else `filesize_approx`). It's the same choice the
  download makes, it needs no network, and it takes about a second. The answer is kept per link, preset and height.
  It's the size downloaded; a re-encode changes the file's size afterwards. Playlists, channels and gallery-dl
  links get no estimate: their listings carry no sizes.
- **Size after:** every download's row keeps `size`: the bytes on disk of what it saved (one file, or the sum of a
  collection's new files). Rows kept from before have none.
- **Downloads at once** is a setting, 10 by default (1–20). It's one pool for the whole app: every single download,
  Grab pick and gallery-dl run takes a slot, and a playlist or channel takes one slot per video, so a channel's
  videos download side by side. A playlist whose listing has every video runs one yt-dlp per video. A longer one
  (more than the 1000 listed, ADR-0016) still runs as one yt-dlp, in one slot. Changing the number applies at once:
  more slots open now, fewer take effect as downloads finish.
- **Speed limit** is a setting in MB/s (MiB per second), 0 for none, the default. It caps each download: yt-dlp and
  gallery-dl get `--limit-rate`, and Rust's own file fetches wait between chunks to stay under it. yt-dlp has no
  limit shared between processes, so ten downloads at 1 MB/s can take 10 MB/s together; the setting says "per
  download". A change applies to downloads that start after it.

## Options considered

- **Estimating sizes in Rust from the probe's formats:** no process to start, but it would have to copy yt-dlp's
  format sorting (`-S`) and fallbacks for every preset; any difference would show the wrong size.
- **One row per video of a playlist:** each video's own progress, but a channel would add a thousand rows and lose
  the playlist's folder, counts and "already saved". Running the videos side by side inside the playlist's job keeps
  the row.
- **A total speed limit:** what people expect from a cap, but it needs a local proxy that all tools download through.
  Dividing the limit among running downloads leaves it wrong as they start and stop.

## Consequences

- A channel downloads up to ten videos at once, and Grab's picks run ten at a time instead of two.
- Ten yt-dlp runs append to the same download archive. A lost line only means that video downloads again next time.
- Each estimate starts yt-dlp once; the window asks only for a single video, once per preset and height.

# ADR-0027: Hunt can remove duplicates when it ends

**Status:** Accepted
**Date:** 2026-11-12
**Amends:** ADR-0013 (a hunt offered a duplicate scan of its folder once done) and ADR-0009 (copies moved to the
trash only after the user looked at them)

## Context

A hunt saves the same picture from several sources in different sizes and formats. The Hunt view offered "Find
duplicates" once done, which means looking through every group by hand. Nari wants a Hunt setting that removes
duplicates by itself, keeping the better copy.

## Decision

- **"Remove duplicates"** is a checkbox in Hunt, on by default (off until 2026-11-12, turned on at the user's request).
  When it's on, the hunt job ends by scanning its own folder (`Images/<topic>/`) with the duplicate finder and moving
  every copy but each group's best to the system trash. The best copy is ADR-0009's: most pixels, then the better
  format, then the higher JPEG quality, then the bigger file.
- **Strict matching only:** copies count as duplicates only when resized or re-saved (ADR-0009's "Strict"). Nobody
  checks the groups first, so a looser match could throw away a different picture.
- **Nothing is deleted outright:** copies go to the system trash and leave the catalog, as with the duplicate panel.
- It runs only when the hunt saved two or more images and wasn't stopped. A failure there doesn't fail the hunt:
  its images stay, and the log says why.
- The hunt's progress says "Removing duplicates" while it runs, and how many it moved once done. The job's file
  count, which the Downloads list shows, is what's left.
- "Find duplicates" stays for a closer look with other strictness levels.

## Options considered

- **Skip a download when its picture is already in the folder:** saves the bandwidth, but the hash needs the file,
  and the first copy to arrive isn't always the best.
- **Normal strictness, as the panel defaults to:** finds more, but also lightly different pictures, with no one to
  check them.

## Consequences

- **Easier:** a hunt ends with one copy of each picture, the best one, without a review.
- **Harder:** the hunt runs longer at the end, hashing every picture it saved (hashes are kept, so a later scan is
  quick). Copies with edits beyond resizing stay; "Find duplicates" catches those.
- **Rules:** `.claude/rules/architecture/core.md`.

// SoundCloud: a Download button at the start of each track's action row (Like, Repost, Share…): the open track's, and
// each track's in a feed or list, sending that track's link (ADR-0035).

import { darkBackground, linkOf, type Place, type Site } from "./site";

// ── Constants ─────────────────────────────
const ROW = ".soundActions .sc-button-group";
/** A track in a feed, search or list, and its title link. */
const ITEM = ".soundList__item, .searchList__item, .trackList__item, .sound";
const ITEM_LINK = "a.soundTitle__title, a.trackItem__trackTitle";

// ── Public API ────────────────────────────
export const soundcloud: Site = {
  look: "soundcloud",
  covers: (host) => host === "soundcloud.com" || host.endsWith(".soundcloud.com"),
  dark: darkBackground,
  places: () => {
    const places: Place[] = [];
    for (const row of document.querySelectorAll(ROW)) {
      // The open track's own row; any other row belongs to a track in a list.
      if (row.closest(".listenEngagement")) {
        places.push({ anchor: row, put: "append", url: linkOf(location.pathname), title: null });
        continue;
      }
      const link = row.closest(ITEM)?.querySelector(ITEM_LINK)?.getAttribute("href");
      if (link) places.push({ anchor: row, put: "append", url: linkOf(link), title: null });
    }
    return places;
  },
};

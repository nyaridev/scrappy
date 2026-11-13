// YouTube: a Download button at the end of a watch page's action row (Like, Share…), sending the video's own link, so
// a video opened from a playlist downloads alone (ADR-0035).

import type { Site } from "./site";

// ── Constants ─────────────────────────────
const ROW = "ytd-watch-metadata #top-level-buttons-computed";

// ── Public API ────────────────────────────
export const youtube: Site = {
  look: "youtube",
  covers: (host) => host === "youtube.com" || host.endsWith(".youtube.com"),
  dark: () => document.documentElement.hasAttribute("dark"),
  places: () => {
    const id = location.pathname === "/watch" ? new URLSearchParams(location.search).get("v") : null;
    const row = document.querySelector(ROW);
    if (!id || !row) return [];
    const title = document.querySelector("ytd-watch-metadata h1")?.textContent?.trim() || null;
    return [{ anchor: row, put: "append", url: `https://www.youtube.com/watch?v=${id}`, title }];
  },
};

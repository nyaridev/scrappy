// YouTube: MP4 and MP3 buttons at the end of a watch page's action row (Like, Share…), sending the video's own link,
// so a video opened from a playlist downloads alone (ADR-0035, ADR-0043).

import type { Site } from "./site";

// ── Constants ─────────────────────────────
const ROW = "ytd-watch-metadata #top-level-buttons-computed";
/** The app's presets the two buttons send. */
const FORMATS = [
  { label: "MP4", preset: "editor-mp4" },
  { label: "MP3", preset: "mp3" },
];

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
    const url = `https://www.youtube.com/watch?v=${id}`;
    return FORMATS.map((format) => ({ anchor: row, put: "append" as const, url, title, format }));
  },
};

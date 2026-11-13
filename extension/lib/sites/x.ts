// X: a Download button before Share on each post that holds a video, sending the post's link (ADR-0035). It finds
// posts and Share by role and label, which outlast X's class names.

import { darkBackground, type Place, type Site } from "./site";

// ── Constants ─────────────────────────────
const POST_LINK = /^\/[^/]+\/status\/\d+/;

// ── Public API ────────────────────────────
export const x: Site = {
  look: "x",
  covers: (host) => ["x.com", "twitter.com", "mobile.x.com", "mobile.twitter.com"].includes(host),
  dark: darkBackground,
  places: () => {
    const places: Place[] = [];
    for (const post of document.querySelectorAll("article")) {
      if (!post.querySelector("video")) continue;
      const share = [...post.querySelectorAll("button[aria-label]")].findLast((button) =>
        /^share/i.test(button.getAttribute("aria-label") ?? ""),
      );
      const path = [...post.querySelectorAll("a[href*='/status/']")]
        .map((link) => link.getAttribute("href")?.match(POST_LINK)?.[0])
        .find(Boolean);
      if (share && path) places.push({ anchor: share, put: "before", url: `https://x.com${path}`, title: null });
    }
    return places;
  },
};

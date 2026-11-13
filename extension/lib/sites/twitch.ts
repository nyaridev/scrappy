// Twitch: a Download button after a clip page's Share button, sending the clip's link (ADR-0035).

import type { Site } from "./site";
import { linkOf } from "./site";

// ── Constants ─────────────────────────────
/** How far up from Share its row's item can be. */
const MAX_DEPTH = 6;

// ── Private helpers ───────────────────────
/** The item in the button row that holds `share`: Share sits a few wrappers deep in it. */
function rowItem(share: Element): Element {
  let item = share;
  for (let depth = 0; depth < MAX_DEPTH && item.parentElement; depth++) {
    if (getComputedStyle(item.parentElement).display.endsWith("flex")) return item;
    item = item.parentElement;
  }
  return share;
}

// ── Public API ────────────────────────────
export const twitch: Site = {
  look: "twitch",
  covers: (host) => host === "twitch.tv" || host.endsWith(".twitch.tv"),
  dark: () => document.documentElement.classList.contains("tw-root--theme-dark"),
  places: () => {
    const clip = location.hostname.startsWith("clips.") || location.pathname.includes("/clip/");
    const share = document.querySelector("button[aria-label^='Share Clip']");
    if (!clip || !share) return [];
    return [{ anchor: rowItem(share), put: "after", url: linkOf(location.pathname), title: null }];
  },
};

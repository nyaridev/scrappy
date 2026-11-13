// Site: what a site's file in `lib/sites/` tells the in-page buttons (ADR-0035): which hosts it covers, its look and
// mode, and where its buttons go on the page as it is now.

import type { Target } from "../buttons/button";
import type { Look } from "../buttons/looks";

// ── Types ─────────────────────────────────
/** Where one button goes: beside `anchor` (`before` or `after` it) or inside it at the end (`append`). */
export interface Place extends Target {
  anchor: Element;
  put: "before" | "after" | "append";
}

/** One supported site. */
export interface Site {
  look: Look;
  /** `location.hostname` belongs to this site. */
  covers: (host: string) => boolean;
  /** The page is in dark mode. */
  dark: () => boolean;
  /** Every place a button belongs on the page now. */
  places: () => Place[];
}

// ── Public API ────────────────────────────
/** The page's background is dark, for sites that mark their mode only with colors. */
export function darkBackground(): boolean {
  const [r = 255, g = 255, b = 255] = (getComputedStyle(document.body).backgroundColor.match(/\d+/g) ?? []).map(Number);
  return 0.2126 * r + 0.7152 * g + 0.0722 * b < 128;
}

/** `path` made absolute on this page's origin, without its query. */
export function linkOf(path: string): string {
  const url = new URL(path, location.href);
  return url.origin + url.pathname;
}

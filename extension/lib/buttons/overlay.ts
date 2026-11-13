// Overlay: on a site without its own file, a Download button in the top corner of the video under the pointer, when
// that video is at least 200 by 120 pixels (ADR-0035). It sends the video's own file when it has a web link; a `blob:`
// stream goes as `null`, for the background script to pick the page or what the tab loaded. YouTube's embedded players
// get it too, sending the video's watch link (ADR-0042).

import { makeButton, type Target } from "./button";

// ── Constants ─────────────────────────────
const MIN_WIDTH = 200;
const MIN_HEIGHT = 120;
const INSET = 10;
/** How long the button stays after the pointer leaves the video, so it can be reached. */
const LINGER_MS = 600;
/** A YouTube player embedded in the page, with its video's id. */
const YOUTUBE_EMBED = /^https?:\/\/(?:www\.)?youtube(?:-nocookie)?\.com\/embed\/([\w-]{11})/;

// ── Types ─────────────────────────────────
/** A video on the page and what its button sends. */
interface Spot {
  element: Element;
  target: Target;
}

// ── Private helpers ───────────────────────
function spotOf(element: Element): Spot | null {
  if (element instanceof HTMLVideoElement) {
    const url = /^https?:/.test(element.currentSrc) ? element.currentSrc : null;
    return { element, target: { url, title: document.title || null } };
  }
  const id = element instanceof HTMLIFrameElement ? element.src.match(YOUTUBE_EMBED)?.[1] : undefined;
  if (!id) return null;
  const title = (element as HTMLIFrameElement).title || null;
  return { element, target: { url: `https://www.youtube.com/watch?v=${id}`, title } };
}

function spotAt(x: number, y: number): Spot | null {
  for (const element of document.querySelectorAll("video, iframe")) {
    const box = element.getBoundingClientRect();
    const big = box.width >= MIN_WIDTH && box.height >= MIN_HEIGHT;
    if (!big || x < box.left || x > box.right || y < box.top || y > box.bottom) continue;
    const spot = spotOf(element);
    if (spot) return spot;
  }
  return null;
}

// ── Public API ────────────────────────────
/** Starts showing the overlay button over videos. */
export function watchVideos() {
  const button = makeButton("overlay", true, { url: null, title: null });
  const { host } = button;
  host.style.cssText = "position:fixed;z-index:2147483647;display:none;";
  document.documentElement.append(host);
  let over: Element | null = null;
  let hide = 0;
  let frame = 0;

  function place(element: Element) {
    const box = element.getBoundingClientRect();
    host.style.display = "inline-flex";
    host.style.top = `${Math.max(box.top, 0) + INSET}px`;
    host.style.left = `${box.right - host.offsetWidth - INSET}px`;
  }

  function follow(x: number, y: number) {
    const spot = spotAt(x, y);
    const onButton = host.matches(":hover");
    if (spot) {
      clearTimeout(hide);
      if (spot.element !== over) button.aim(spot.target);
      over = spot.element;
      place(spot.element);
    } else if (over && !onButton) {
      clearTimeout(hide);
      hide = window.setTimeout(() => {
        host.style.display = "none";
        over = null;
      }, LINGER_MS);
    }
  }

  const onPointer = (event: PointerEvent) => {
    cancelAnimationFrame(frame);
    frame = requestAnimationFrame(() => follow(event.clientX, event.clientY));
  };
  document.addEventListener("pointermove", onPointer, { passive: true });
  // Moves inside another site's frame never reach the page: entering the frame is the last thing it sees.
  document.addEventListener("pointerover", onPointer, { passive: true });
  document.addEventListener("scroll", () => over && place(over), { passive: true, capture: true });
}

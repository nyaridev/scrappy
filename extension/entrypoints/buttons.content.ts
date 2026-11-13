// Buttons: adds Download buttons to pages (ADR-0035). A supported site (`lib/sites/`) gets them in its own places and
// look, kept up as the page changes without reloading; any other site gets the overlay on its videos.

import { makeButton, type PageButton } from "../lib/buttons/button";
import { watchVideos } from "../lib/buttons/overlay";
import { SITES } from "../lib/sites";
import type { Site } from "../lib/sites/site";

// ── Constants ─────────────────────────────
/** A busy page changes many times a second: places are looked for again at most this often. */
const SETTLE_MS = 400;

// ── Private helpers ───────────────────────
function watchSite(site: Site) {
  const placed = new WeakMap<Element, PageButton>();
  let waiting = 0;

  function update() {
    waiting = 0;
    const dark = site.dark();
    for (const { anchor, put, url, title } of site.places()) {
      const known = placed.get(anchor);
      if (known?.host.isConnected) {
        known.aim({ url, title });
        continue;
      }
      const button = makeButton(site.look, dark, { url, title });
      placed.set(anchor, button);
      if (put === "append") anchor.append(button.host);
      else if (put === "before") anchor.before(button.host);
      else anchor.after(button.host);
    }
  }

  new MutationObserver(() => {
    if (!waiting) waiting = window.setTimeout(update, SETTLE_MS);
  }).observe(document.body, { childList: true, subtree: true });
  update();
}

export default defineContentScript({
  matches: ["<all_urls>"],
  runAt: "document_idle",
  main() {
    const site = SITES.find((one) => one.covers(location.hostname));
    if (site) watchSite(site);
    else watchVideos();
  },
});

// usePreview: a listed video's thumbnail from Rust, asked for once its element scrolls into view and kept for the
// session, so a list of hundreds only loads what's seen.

import { useCallback, useEffect, useState } from "react";
import { entryThumbnail } from "../../lib/backend";

// ── State ─────────────────────────────────
/** By video link: the `data:` URL, or `null` when it has none. */
const previews = new Map<string, string | null>();
/** One observer for every card; each element's callback runs once, the first time it shows. */
const waiting = new Map<Element, () => void>();
let observer: IntersectionObserver | null = null;

// ── Private helpers ───────────────────────
function watch(element: Element, onSeen: () => void): () => void {
  observer ??= new IntersectionObserver(
    (seen) => {
      for (const entry of seen) {
        if (!entry.isIntersecting) continue;
        waiting.get(entry.target)?.();
        waiting.delete(entry.target);
        observer?.unobserve(entry.target);
      }
    },
    { rootMargin: "200px" },
  );
  waiting.set(element, onSeen);
  observer.observe(element);
  return () => {
    waiting.delete(element);
    observer?.unobserve(element);
  };
}

// ── Public API ────────────────────────────
/** A ref for the element that shows it, and the picture: `undefined` until it loads, `null` when there's none. */
export function usePreview(url: string): [(element: Element | null) => void, string | null | undefined] {
  const [src, setSrc] = useState(() => previews.get(url));
  const [element, setElement] = useState<Element | null>(null);
  useEffect(() => {
    if (previews.has(url)) return setSrc(previews.get(url));
    if (!element) return;
    let current = true;
    const stop = watch(element, () =>
      entryThumbnail(url)
        .catch(() => null)
        .then((fresh) => {
          previews.set(url, fresh);
          if (current) setSrc(fresh);
        }),
    );
    return () => {
      current = false;
      stop();
    };
  }, [url, element]);
  return [useCallback((next: Element | null) => setElement(next), []), src];
}

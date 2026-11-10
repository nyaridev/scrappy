// useGallery: the gallery's files for a filter and sort, a page at a time; `loadMore` adds the next page. The last list
// is kept between visits, so the view opens with it while a fresh copy loads; `reload` starts over.

import { useCallback, useEffect, useRef, useState } from "react";
import { galleryPage } from "../../lib/backend";
import type { GalleryItem } from "../../lib/types/GalleryItem";
import type { GallerySort } from "../../lib/types/GallerySort";

// ── Constants ─────────────────────────────
const PAGE = 60;

// ── Types ─────────────────────────────────
export interface GalleryFilter {
  kind: string | null;
  site: string | null;
  search: string;
  sort: GallerySort;
}

// ── State ─────────────────────────────────
let last: { key: string; items: GalleryItem[]; total: number } | null = null;

// ── Public API ────────────────────────────
/** The files so far, how many match in all (`null` before the first page), and loading more. */
export function useGallery(filter: GalleryFilter) {
  const key = JSON.stringify(filter);
  const kept = last?.key === key ? last : null;
  const [items, setItems] = useState<GalleryItem[]>(kept?.items ?? []);
  const [total, setTotal] = useState<number | null>(kept?.total ?? null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  // Answers to an older filter arrive late; only the newest one counts.
  const ticket = useRef(0);

  const load = useCallback(
    async (offset: number) => {
      const mine = ++ticket.current;
      setLoading(true);
      setError(null);
      try {
        const page = await galleryPage({ ...(JSON.parse(key) as GalleryFilter), offset, limit: PAGE });
        if (mine !== ticket.current) return;
        setItems((all) => {
          const fresh = offset === 0 ? page.items : [...all, ...page.items];
          last = { key, items: fresh, total: page.total };
          return fresh;
        });
        setTotal(page.total);
      } catch (e) {
        if (mine === ticket.current) setError(String(e));
      } finally {
        if (mine === ticket.current) setLoading(false);
      }
    },
    [key],
  );

  useEffect(() => void load(0), [load]);

  const loadMore = useCallback(() => {
    if (!loading && total != null && items.length < total) void load(items.length);
  }, [loading, total, items.length, load]);

  const reload = useCallback(() => void load(0), [load]);

  return { items, total, loading, error, loadMore, reload };
}

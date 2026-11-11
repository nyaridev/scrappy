// useGallery: the gallery's files for a filter and sort, a page at a time; `loadMore` adds the next page. The last list
// is kept between visits, so the view opens with it while a fresh copy loads; `reload` starts over. When the library
// folder changes, the rows shown are read again in place.

import { useCallback, useEffect, useRef, useState } from "react";
import { useViewActive } from "../../app/viewShown";
import { galleryPage, onLibraryChanged } from "../../lib/backend";
import type { GalleryItem } from "../../lib/types/GalleryItem";
import type { GallerySort } from "../../lib/types/GallerySort";

// ── Constants ─────────────────────────────
const PAGE = 60;
/** The most rows a refresh reads again (the core's page limit). */
const MAX_REFRESH = 1000;

// ── Types ─────────────────────────────────
export interface GalleryFilter {
  kind: string | null;
  site: string | null;
  search: string;
  sort: GallerySort;
}

// ── State ─────────────────────────────────
let last: { key: string; items: GalleryItem[]; total: number; kinds: string[]; sites: string[] } | null = null;

// ── Public API ────────────────────────────
/** The files so far, how many match in all (`null` before the first page), and loading more. */
export function useGallery(filter: GalleryFilter) {
  const key = JSON.stringify(filter);
  const kept = last?.key === key ? last : null;
  const [items, setItems] = useState<GalleryItem[]>(kept?.items ?? []);
  const [total, setTotal] = useState<number | null>(kept?.total ?? null);
  const [kinds, setKinds] = useState<string[]>(kept?.kinds ?? []);
  const [sites, setSites] = useState<string[]>(kept?.sites ?? []);
  const shown = useRef(items.length);
  shown.current = items.length;
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  // Answers to an older filter arrive late; only the newest one counts.
  const ticket = useRef(0);

  const load = useCallback(
    async (offset: number, limit = PAGE) => {
      const mine = ++ticket.current;
      setLoading(true);
      setError(null);
      try {
        const page = await galleryPage({ ...(JSON.parse(key) as GalleryFilter), offset, limit });
        if (mine !== ticket.current) return;
        setItems((all) => {
          const fresh = offset === 0 ? page.items : [...all, ...page.items];
          last = { key, items: fresh, total: page.total, kinds: page.kinds, sites: page.sites };
          return fresh;
        });
        setTotal(page.total);
        setKinds(page.kinds);
        setSites(page.sites);
      } catch (e) {
        if (mine === ticket.current) setError(String(e));
      } finally {
        if (mine === ticket.current) setLoading(false);
      }
    },
    [key],
  );

  useEffect(() => void load(0), [load]);

  // A change in the folder reads again every row on screen, so the grid keeps its place. While the Gallery is hidden
  // (a grab saving a thousand files), changes only mark it out of date, and it reads again when it shows.
  const active = useViewActive();
  const activeNow = useRef(active);
  const stale = useRef(false);
  activeNow.current = active;
  const refresh = useCallback(() => void load(0, Math.min(MAX_REFRESH, Math.max(PAGE, shown.current))), [load]);
  useEffect(
    () =>
      onLibraryChanged(() => {
        if (activeNow.current) refresh();
        else stale.current = true;
      }),
    [refresh],
  );
  useEffect(() => {
    if (!active || !stale.current) return;
    stale.current = false;
    refresh();
  }, [active, refresh]);

  const loadMore = useCallback(() => {
    if (!loading && total != null && items.length < total) void load(items.length);
  }, [loading, total, items.length, load]);

  const reload = useCallback(() => void load(0), [load]);

  return { items, total, kinds, sites, loading, error, loadMore, reload };
}

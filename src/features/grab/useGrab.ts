// useGrab: the page Grab shows (scanned here, or handed over by the extension) and which download each picked find
// became. Both are kept between visits; how each download is doing comes from `useDownloads`.

import { useCallback, useEffect, useState } from "react";
import { grabFinds, scanPage } from "../../lib/backend";
import type { Find } from "../../lib/types/Find";
import type { PageScan } from "../../lib/types/PageScan";
import type { Picked } from "../../lib/types/Picked";

// ── State ─────────────────────────────────
let lastPage: PageScan | null = null;
let lastPicked: Record<string, Picked> = {};
const pageListeners = new Set<(page: PageScan) => void>();

// ── Public API ────────────────────────────
/** Shows `page` in Grab, replacing the one there. */
export function showPage(page: PageScan) {
  lastPage = page;
  lastPicked = {};
  for (const listener of pageListeners) listener(page);
}

/** The page, its picks by link, and scanning and grabbing. */
export function useGrab() {
  const [page, setPage] = useState(lastPage);
  const [picked, setPicked] = useState(lastPicked);

  useEffect(() => {
    const onPage = (fresh: PageScan) => {
      setPage(fresh);
      setPicked({});
    };
    pageListeners.add(onPage);
    return () => void pageListeners.delete(onPage);
  }, []);

  const scan = useCallback(async (url: string) => showPage(await scanPage(url)), []);

  const grab = useCallback(
    async (finds: Find[]) => {
      if (!page) return;
      const picks = finds.map(({ url, kind, name }) => ({ url, kind, name }));
      const results = await grabFinds({ pageUrl: page.url, title: page.title, finds: picks });
      setPicked((all) => {
        lastPicked = { ...all, ...Object.fromEntries(results.map((result) => [result.url, result])) };
        return lastPicked;
      });
    },
    [page],
  );

  return { page, picked, scan, grab };
}

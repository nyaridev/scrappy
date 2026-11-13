// useGrab: the page Grab shows (scanned here, or handed over by the extension) and which download each picked find
// became. Both are kept between visits; how each download is doing comes from `useDownloads`. A scan is a job: its
// finds show as they arrive (ADR-0019).

import { useCallback, useEffect, useRef, useState } from "react";
import { cancelJob, grabFinds, onJobFinished, onJobProgress, scannedPage, startScan } from "../../lib/backend";
import { coalesce } from "../../lib/coalesce";
import type { Find } from "../../lib/types/Find";
import type { PageScan } from "../../lib/types/PageScan";
import type { Picked } from "../../lib/types/Picked";
import type { ScanRequest } from "../../lib/types/ScanRequest";

// ── Constants ─────────────────────────────
/** How often a running scan's finds are read again; its reports come every half second. */
const READ_EVERY_MS = 250;

// ── Types ─────────────────────────────────
interface Running {
  id: number;
  end: (error?: string) => void;
}

// ── State ─────────────────────────────────
let lastPage: PageScan | null = null;
let lastPicked: Record<string, Picked> = {};
/** Counts new pages, so the view resets its filter and picks for a new page, not for each update of a scan. */
let lastFresh = 0;
const pageListeners = new Set<(page: PageScan, fresh: boolean) => void>();

// ── Public API ────────────────────────────
/** Shows `page` in Grab: a new page (`fresh`) replaces the one there and its picks; otherwise it's the same page with
 * more finds. */
export function showPage(page: PageScan, fresh = true) {
  lastPage = page;
  if (fresh) {
    lastPicked = {};
    lastFresh += 1;
  }
  for (const listener of pageListeners) listener(page, fresh);
}

/** The page, a count that changes with each new page, its picks by link, and scanning, stopping and grabbing. */
export function useGrab() {
  const [page, setPage] = useState(lastPage);
  const [fresh, setFresh] = useState(lastFresh);
  const [picked, setPicked] = useState(lastPicked);
  const running = useRef<Running | null>(null);

  useEffect(() => {
    const onPage = (next: PageScan, isFresh: boolean) => {
      setPage(next);
      if (!isFresh) return;
      setPicked({});
      setFresh(lastFresh);
    };
    pageListeners.add(onPage);
    const read = coalesce(() => {
      const id = running.current?.id;
      if (id == null) return;
      void scannedPage(id).then((next) => {
        if (next && running.current?.id === id) showPage(next, false);
      });
    }, READ_EVERY_MS);
    const stopProgress = onJobProgress((report) => {
      if (report.id === running.current?.id) read();
    });
    const stopFinished = onJobFinished(({ id, result }) => {
      const scan = running.current;
      if (id !== scan?.id) return;
      // A stopped scan keeps what it found.
      void scannedPage(id).then((next) => {
        if (running.current !== scan) return;
        running.current = null;
        if (next) showPage(next, false);
        scan.end(result.state === "failed" ? result.message : undefined);
      });
    });
    return () => {
      pageListeners.delete(onPage);
      stopProgress();
      stopFinished();
    };
  }, []);

  /** Scans the request's page (and, deeper, the pages it links to); resolves when the scan ends, rejects with what
   * failed. */
  const scan = useCallback(
    (request: ScanRequest) =>
      new Promise<void>((resolve, reject) => {
        startScan(request).then((id) => {
          running.current = { id, end: (error) => (error ? reject(error) : resolve()) };
          showPage({ url: request.url, title: null, finds: [], pages: 0, kept: {} });
        }, reject);
      }),
    [],
  );

  const stop = useCallback(() => {
    const id = running.current?.id;
    if (id != null) void cancelJob(id);
  }, []);

  const grab = useCallback(
    async (finds: Find[], removeDuplicates: boolean) => {
      if (!page) return;
      const picks = finds.map(({ url, kind, name }) => ({ url, kind, name }));
      const results = await grabFinds({ pageUrl: page.url, title: page.title, finds: picks, removeDuplicates });
      setPicked((all) => {
        lastPicked = { ...all, ...Object.fromEntries(results.map((result) => [result.url, result])) };
        return lastPicked;
      });
    },
    [page],
  );

  return { page, fresh, picked, scan, stop, grab };
}

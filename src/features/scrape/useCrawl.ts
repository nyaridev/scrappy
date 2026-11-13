// useCrawl: one crawl, mirror or page save at a time: start it, follow its job (files saved, pages visited) and its
// end. The job is followed outside the view, so one that ends while another view is open shows its end on return.

import { useCallback, useEffect, useState } from "react";
import { cancelJob, crawlSite, onJobFinished, onJobProgress } from "../../lib/backend";
import type { CrawlMode } from "../../lib/types/CrawlMode";
import type { CrawlRequest } from "../../lib/types/CrawlRequest";
import type { CrawlStarted } from "../../lib/types/CrawlStarted";
import type { JobProgress } from "../../lib/types/JobProgress";

// ── Types ─────────────────────────────────
/** Where a started crawl saves to, and its mode. */
type Started = Omit<CrawlStarted, "id"> & { mode: CrawlMode };

export type CrawlState =
  | { state: "idle" }
  | ({ state: "running"; id: number; progress: JobProgress | null } & Started)
  | ({ state: "done"; progress: JobProgress | null } & Started)
  | ({ state: "stopped"; progress: JobProgress | null } & Started)
  | { state: "failed"; message: string };

// ── State ─────────────────────────────────
let last: CrawlState = { state: "idle" };
const watchers = new Set<(state: CrawlState) => void>();

// ── Private helpers ───────────────────────
function set(next: CrawlState) {
  last = next;
  for (const watcher of watchers) watcher(next);
}

/** Follows job `id` until it ends. */
function follow(id: number, started: Started) {
  let progress: JobProgress | null = null;
  const stopProgress = onJobProgress((report) => {
    if (report.id !== id) return;
    progress = report;
    set({ state: "running", id, ...started, progress });
  });
  const stopFinished = onJobFinished(({ id: ended, result }) => {
    if (ended !== id) return;
    stopProgress();
    stopFinished();
    if (result.state === "failed") set({ state: "failed", message: result.message });
    else set({ state: result.state === "done" ? "done" : "stopped", ...started, progress });
  });
}

// ── Public API ────────────────────────────
/** The crawl and its actions. */
export function useCrawl() {
  const [crawl, setCrawl] = useState<CrawlState>(last);

  useEffect(() => {
    watchers.add(setCrawl);
    return () => void watchers.delete(setCrawl);
  }, []);

  const start = useCallback(async (request: CrawlRequest) => {
    try {
      const { id, site, folder, open } = await crawlSite(request);
      const started = { site, folder, open, mode: request.mode };
      set({ state: "running", id, ...started, progress: null });
      follow(id, started);
    } catch (e) {
      set({ state: "failed", message: String(e) });
    }
  }, []);

  const stop = useCallback(() => {
    if (last.state === "running") void cancelJob(last.id);
  }, []);

  return { crawl, start, stop };
}

// useHunt: one hunt at a time: start it, follow its job (images saved of the limit, the site being read) and its end.
// The job is followed outside the view, so a hunt that ends while another view is open shows its end on return.

import { useCallback, useEffect, useState } from "react";
import { cancelJob, onJobFinished, onJobProgress, startHunt } from "../../lib/backend";
import type { HuntRequest } from "../../lib/types/HuntRequest";
import type { JobProgress } from "../../lib/types/JobProgress";

// ── Types ─────────────────────────────────
export type HuntState =
  | { state: "idle" }
  | { state: "running"; id: number; folder: string; progress: JobProgress | null }
  | { state: "done"; folder: string; progress: JobProgress | null }
  | { state: "stopped"; folder: string; progress: JobProgress | null }
  | { state: "failed"; message: string };

// ── State ─────────────────────────────────
let last: HuntState = { state: "idle" };
const watchers = new Set<(state: HuntState) => void>();

// ── Private helpers ───────────────────────
function set(next: HuntState) {
  last = next;
  for (const watcher of watchers) watcher(next);
}

/** Follows job `id` until it ends. */
function follow(id: number, folder: string) {
  let progress: JobProgress | null = null;
  const stopProgress = onJobProgress((report) => {
    if (report.id !== id) return;
    // A save's report has no message; keep the site from the last one.
    progress = { ...report, message: report.message ?? progress?.message ?? null };
    set({ state: "running", id, folder, progress });
  });
  const stopFinished = onJobFinished(({ id: ended, result }) => {
    if (ended !== id) return;
    stopProgress();
    stopFinished();
    if (result.state === "failed") set({ state: "failed", message: result.message });
    else set({ state: result.state === "done" ? "done" : "stopped", folder, progress });
  });
}

// ── Public API ────────────────────────────
/** The hunt and its actions. */
export function useHunt() {
  const [hunt, setHunt] = useState<HuntState>(last);

  useEffect(() => {
    watchers.add(setHunt);
    return () => void watchers.delete(setHunt);
  }, []);

  const start = useCallback(async (request: HuntRequest) => {
    try {
      const { id, folder } = await startHunt(request);
      set({ state: "running", id, folder, progress: null });
      follow(id, folder);
    } catch (e) {
      set({ state: "failed", message: String(e) });
    }
  }, []);

  const stop = useCallback(() => {
    if (last.state === "running") void cancelJob(last.id);
  }, []);

  return { hunt, start, stop };
}

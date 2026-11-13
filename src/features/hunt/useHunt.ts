// useHunt: one hunt at a time: start it, follow its job (images saved of the total) and its status (each source's
// progress, then the duplicates removed), read from Rust as job events arrive (ADR-0024, ADR-0027), and its end. The job is followed outside the view, so a hunt that ends
// while another view is open shows its end on return.

import { useCallback, useEffect, useState } from "react";
import { cancelJob, huntStatus, onJobFinished, onJobProgress, startHunt } from "../../lib/backend";
import { coalesce } from "../../lib/coalesce";
import type { HuntRequest } from "../../lib/types/HuntRequest";
import type { HuntStatus } from "../../lib/types/HuntStatus";
import type { JobProgress } from "../../lib/types/JobProgress";

// ── Constants ─────────────────────────────
/** How often the hunt's status is read while the job reports. */
const READ_EVERY_MS = 250;
const NO_STATUS: HuntStatus = { sources: [], removing: false, removed: null };

// ── Types ─────────────────────────────────
export type HuntState =
  | { state: "idle" }
  | { state: "running"; id: number; folder: string; progress: JobProgress | null; status: HuntStatus }
  | { state: "done"; folder: string; progress: JobProgress | null; status: HuntStatus }
  | { state: "stopped"; folder: string; progress: JobProgress | null; status: HuntStatus }
  | { state: "failed"; message: string; status: HuntStatus };

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
  let status = NO_STATUS;
  const running = () => last.state === "running" && last.id === id;
  const read = coalesce(() => {
    void huntStatus(id).then((next) => {
      if (!next || !running()) return;
      status = next;
      set({ state: "running", id, folder, progress, status });
    });
  }, READ_EVERY_MS);
  const stopProgress = onJobProgress((report) => {
    if (report.id !== id) return;
    progress = report;
    set({ state: "running", id, folder, progress, status });
    read();
  });
  const stopFinished = onJobFinished(({ id: ended, result }) => {
    if (ended !== id) return;
    stopProgress();
    stopFinished();
    // The sources as they ended, so each one's last state shows.
    void huntStatus(id).then((final) => {
      status = final ?? status;
      if (result.state === "failed") set({ state: "failed", message: result.message, status });
      else set({ state: result.state === "done" ? "done" : "stopped", folder, progress, status });
    });
  });
  // The sources, all waiting, before the first report.
  read();
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
      set({ state: "running", id, folder, progress: null, status: NO_STATUS });
      follow(id, folder);
    } catch (e) {
      set({ state: "failed", message: String(e), status: NO_STATUS });
    }
  }, []);

  const stop = useCallback(() => {
    if (last.state === "running") void cancelJob(last.id);
  }, []);

  return { hunt, start, stop };
}

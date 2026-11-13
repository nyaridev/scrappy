// useDuplicates: one duplicate scan at a time: start it, follow its job, take the groups when it ends, and move the
// picked copies to the trash. Every copy but each group's best starts picked. A scan that ends quickly (every picture
// hashed before) keeps the last results on screen, so a rerun doesn't flash the progress for a frame.

import { useCallback, useEffect, useRef, useState } from "react";
import { cancelJob, duplicatesFound, findDuplicates, onJobFinished, onJobProgress, trashFiles } from "../lib/backend";
import type { JobProgress } from "../lib/types/JobProgress";
import type { JobResult } from "../lib/types/JobResult";
import type { Picture } from "../lib/types/Picture";
import type { Strictness } from "../lib/types/Strictness";

// ── Constants ─────────────────────────────
/** How long a scan runs before its progress shows. */
const SHOW_AFTER_MS = 300;

// ── Types ─────────────────────────────────
export type ScanResult =
  | { state: "idle" }
  | { state: "done"; groups: Picture[][] }
  | { state: "failed"; message: string };

// ── Public API ────────────────────────────
/** The scan of the whole library or `folder`, the picked paths, and the actions. */
export function useDuplicates(folder?: string) {
  const [result, setResult] = useState<ScanResult>({ state: "idle" });
  const [picked, setPicked] = useState<Set<string>>(new Set());
  const [busy, setBusy] = useState(false);
  // `busy` once it has lasted SHOW_AFTER_MS: what the panel shows as running.
  const [running, setRunning] = useState(false);
  const [progress, setProgress] = useState<JobProgress | null>(null);
  // The scan's job, once its id is known.
  const job = useRef<number | null>(null);
  // A scan can end before `findDuplicates` hands back its id (nothing to hash, or every picture hashed before):
  // jobs that end while a start waits are kept, so that end isn't missed.
  const starting = useRef(false);
  const endedEarly = useRef(new Map<number, JobResult>());

  useEffect(() => {
    if (!busy) {
      setRunning(false);
      return;
    }
    const timer = setTimeout(() => setRunning(true), SHOW_AFTER_MS);
    return () => clearTimeout(timer);
  }, [busy]);

  const finish = useCallback(async (id: number, ended: JobResult) => {
    job.current = null;
    if (ended.state === "failed") setResult({ state: "failed", message: ended.message });
    else if (ended.state === "cancelled") setResult({ state: "idle" });
    else {
      const groups = (await duplicatesFound(id)) ?? [];
      setPicked(new Set(groups.flatMap((group) => group.slice(1).map((picture) => picture.path))));
      setResult({ state: "done", groups });
    }
    setProgress(null);
    setBusy(false);
  }, []);

  // Listening from the start, not from when the id arrives: a fast scan's events come before that.
  useEffect(() => {
    const stopProgress = onJobProgress((report) => {
      if (report.id === job.current) setProgress(report);
    });
    const stopFinished = onJobFinished(({ id, result: ended }) => {
      if (id === job.current) void finish(id, ended);
      else if (starting.current) endedEarly.current.set(id, ended);
    });
    return () => {
      stopProgress();
      stopFinished();
    };
  }, [finish]);

  const start = useCallback(
    async (strictness: Strictness) => {
      // One scan at a time: clicks while one runs are ignored.
      if (starting.current || job.current != null) return;
      starting.current = true;
      endedEarly.current.clear();
      setBusy(true);
      try {
        const id = await findDuplicates({ folder: folder ?? null, strictness });
        const ended = endedEarly.current.get(id);
        if (ended) return void finish(id, ended);
        job.current = id;
      } catch (e) {
        setResult({ state: "failed", message: String(e) });
        setBusy(false);
      } finally {
        starting.current = false;
        endedEarly.current.clear();
      }
    },
    [folder, finish],
  );

  const toggle = useCallback((path: string, on: boolean) => {
    setPicked((all) => {
      const next = new Set(all);
      if (on) next.add(path);
      else next.delete(path);
      return next;
    });
  }, []);

  /** Moves the picked copies to the trash and drops them from the groups; returns how many moved. */
  const trashPicked = useCallback(async () => {
    const paths = [...picked];
    const moved = await trashFiles(paths);
    const gone = new Set(paths);
    setPicked(new Set());
    setResult((current) =>
      current.state === "done"
        ? {
            state: "done",
            groups: current.groups
              .map((group) => group.filter((picture) => !gone.has(picture.path)))
              .filter((group) => group.length > 1),
          }
        : current,
    );
    return moved;
  }, [picked]);

  const stop = useCallback(() => {
    if (job.current != null) void cancelJob(job.current);
  }, []);

  return { result, running, progress, picked, start, stop, toggle, trashPicked };
}

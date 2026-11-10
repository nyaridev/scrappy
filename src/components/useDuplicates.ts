// useDuplicates: one duplicate scan at a time: start it, follow its job, take the groups when it ends, and move the
// picked copies to the trash. Every copy but each group's best starts picked.

import { useCallback, useEffect, useState } from "react";
import { cancelJob, duplicatesFound, findDuplicates, onJobFinished, onJobProgress, trashFiles } from "../lib/backend";
import type { JobProgress } from "../lib/types/JobProgress";
import type { Picture } from "../lib/types/Picture";
import type { Strictness } from "../lib/types/Strictness";

// ── Types ─────────────────────────────────
export type ScanState =
  | { state: "idle" }
  | { state: "running"; id: number; progress: JobProgress | null }
  | { state: "done"; groups: Picture[][] }
  | { state: "failed"; message: string };

// ── Public API ────────────────────────────
/** The scan of the whole library or `folder`, the picked paths, and the actions. */
export function useDuplicates(folder?: string) {
  const [scan, setScan] = useState<ScanState>({ state: "idle" });
  const [picked, setPicked] = useState<Set<string>>(new Set());
  const running = scan.state === "running" ? scan.id : null;

  useEffect(() => {
    if (running == null) return;
    const stopProgress = onJobProgress((progress) => {
      if (progress.id === running) setScan({ state: "running", id: running, progress });
    });
    const stopFinished = onJobFinished(async ({ id, result }) => {
      if (id !== running) return;
      if (result.state === "failed") return setScan({ state: "failed", message: result.message });
      if (result.state === "cancelled") return setScan({ state: "idle" });
      const groups = (await duplicatesFound(id)) ?? [];
      setPicked(new Set(groups.flatMap((group) => group.slice(1).map((picture) => picture.path))));
      setScan({ state: "done", groups });
    });
    return () => {
      stopProgress();
      stopFinished();
    };
  }, [running]);

  const start = useCallback(
    async (strictness: Strictness) => {
      try {
        const id = await findDuplicates({ folder: folder ?? null, strictness });
        setScan({ state: "running", id, progress: null });
      } catch (e) {
        setScan({ state: "failed", message: String(e) });
      }
    },
    [folder],
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
    setScan((current) =>
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
    if (running != null) void cancelJob(running);
  }, [running]);

  return { scan, picked, start, stop, toggle, trashPicked };
}

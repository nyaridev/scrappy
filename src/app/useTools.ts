// useTools: the tools' status from Rust, installs started from the window, and their progress and errors from job
// events. The first-run tools step and Settings both use it. The last list Rust sent is kept, so a view opened again
// shows it at once while a fresh one loads.

import { useCallback, useEffect, useState } from "react";
import { cancelJob, installTools, onJobFinished, onJobProgress, toolsStatus } from "../lib/backend";
import type { JobProgress } from "../lib/types/JobProgress";
import type { ToolStatus } from "../lib/types/ToolStatus";

// ── Types ─────────────────────────────────
/** One tool with what's happening to it. */
export interface ToolRow {
  status: ToolStatus;
  /** The running install's last report. */
  progress: JobProgress | null;
  /** Why its last install failed. */
  error: string | null;
}

// ── State ─────────────────────────────────
/** The last list Rust sent, shared by every view that shows the tools. */
let lastStatuses: ToolStatus[] | null = null;

// ── Public API ────────────────────────────
/** The tool rows (`null` until Rust answers), whether any can be downloaded or is downloading, and the actions. */
export function useTools() {
  const [statuses, setStatuses] = useState<ToolStatus[] | null>(lastStatuses);
  const [progress, setProgress] = useState<Record<number, JobProgress>>({});
  const [errors, setErrors] = useState<Record<number, string>>({});
  // The last job each tool had, so an error stays on its row after the job is gone.
  const [lastJob, setLastJob] = useState<Record<string, number>>({});

  const refresh = useCallback(
    () =>
      void toolsStatus().then((fresh) => {
        lastStatuses = fresh;
        setStatuses(fresh);
      }),
    [],
  );

  useEffect(() => {
    refresh();
    const stopProgress = onJobProgress((report) => setProgress((all) => ({ ...all, [report.id]: report })));
    const stopFinished = onJobFinished(({ id, result }) => {
      if (result.state === "failed") setErrors((all) => ({ ...all, [id]: result.message }));
      refresh();
    });
    return () => {
      stopProgress();
      stopFinished();
    };
  }, [refresh]);

  const install = useCallback(async () => {
    const started = await installTools();
    setLastJob((all) => Object.fromEntries([...Object.entries(all), ...started.map((job) => [job.tool, job.id])]));
    setStatuses(
      (all) => all?.map((s) => ({ ...s, job: started.find((job) => job.tool === s.tool)?.id ?? s.job })) ?? null,
    );
  }, []);

  const cancel = useCallback(() => {
    for (const status of statuses ?? []) if (status.job !== null) void cancelJob(status.job);
  }, [statuses]);

  const rows: ToolRow[] | null =
    statuses?.map((status) => {
      const job = status.job ?? lastJob[status.tool];
      return {
        status,
        progress: status.job !== null ? (progress[status.job] ?? null) : null,
        error: status.job === null && job !== undefined ? (errors[job] ?? null) : null,
      };
    }) ?? null;

  return {
    rows,
    /** For the first-run step: a tool is missing, or Scrappy's own copy is outdated, and Scrappy can download it. An
     * outdated copy on PATH doesn't bring the step back on every start; Settings offers its update. */
    needed:
      statuses?.some((s) => s.downloadable && (s.source === "missing" || (s.outdated && s.source === "scrappy"))) ??
      false,
    /** A tool is missing or outdated, and Scrappy can download it. */
    updatable: statuses?.some((s) => s.downloadable && (s.source === "missing" || s.outdated)) ?? false,
    /** A tool Scrappy can download is missing, so the button says Download rather than Update. */
    missing: statuses?.some((s) => s.downloadable && s.source === "missing") ?? false,
    installing: statuses?.some((s) => s.job !== null) ?? false,
    install,
    cancel,
  };
}

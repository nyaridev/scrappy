// useDownloads: this session's downloads from Rust, with each one's progress from job events. The list is kept
// between visits, so the view opens with it. Downloads the extension sends show up when their first progress arrives.

import { useCallback, useEffect, useState } from "react";
import { cancelJob, downloads, onJobFinished, onJobProgress, startDownload } from "../../lib/backend";
import type { DownloadEntry } from "../../lib/types/DownloadEntry";
import type { DownloadRequest } from "../../lib/types/DownloadRequest";
import type { JobProgress } from "../../lib/types/JobProgress";

// ── State ─────────────────────────────────
let lastEntries: DownloadEntry[] = [];
let lastProgress: Record<number, JobProgress> = {};
/** Job ids the list was reloaded for already; tool installs report progress too and aren't downloads. */
const lookedUp = new Set<number>();

// ── Public API ────────────────────────────
/** The downloads, newest first, their progress by id, and the actions. */
export function useDownloads() {
  const [entries, setEntries] = useState(lastEntries);
  const [progress, setProgress] = useState(lastProgress);

  const keep = useCallback((fresh: DownloadEntry[]) => {
    lastEntries = fresh;
    setEntries(fresh);
  }, []);

  useEffect(() => {
    void downloads().then(keep);
    const stopProgress = onJobProgress((report) => {
      if (!lastEntries.some((entry) => entry.id === report.id) && !lookedUp.has(report.id)) {
        lookedUp.add(report.id);
        void downloads().then(keep);
      }
      setProgress((all) => {
        lastProgress = { ...all, [report.id]: report };
        return lastProgress;
      });
    });
    const stopFinished = onJobFinished(() => void downloads().then(keep));
    return () => {
      stopProgress();
      stopFinished();
    };
  }, [keep]);

  const start = useCallback(
    async (request: DownloadRequest) => keep([...lastEntries, await startDownload(request)]),
    [keep],
  );

  return { entries: [...entries].reverse(), progress, start, cancel: (id: number) => void cancelJob(id) };
}

// useLinkCheck: the Download view's link check, a job (ADR-0030). What Rust has found so far shows as it grows, read in
// batches as the job reports; Stop ends it and keeps what was listed. A check that ends before its id arrives (a link
// checked within the hour) is caught from the jobs that ended meanwhile.

import { useCallback, useEffect, useRef, useState } from "react";
import { cancelJob, listedLink, listLink, onJobFinished, onJobProgress } from "../../lib/backend";
import { coalesce } from "../../lib/coalesce";
import type { JobResult } from "../../lib/types/JobResult";
import type { Probe } from "../../lib/types/Probe";

// ── Constants ─────────────────────────────
/** How often a running check's list is read again; its reports come every 100 ms. */
const READ_EVERY_MS = 250;
/** How many ended jobs are remembered, for a check that ends before its id arrives. */
const ENDED_KEPT = 20;

// ── Public API ────────────────────────────
/** The probe so far, whether the check still runs, what failed, and checking, stopping and clearing. */
export function useLinkCheck() {
  const [probe, setProbe] = useState<Probe | null>(null);
  const [running, setRunning] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const current = useRef<number | null>(null);
  const ended = useRef(new Map<number, JobResult>());

  const finish = useCallback((id: number, result: JobResult) => {
    // A stopped check keeps what it listed.
    void listedLink(id).then((found) => {
      if (current.current !== id) return;
      current.current = null;
      setRunning(false);
      if (found) setProbe(found);
      if (result.state === "failed") setError(result.message);
    });
  }, []);

  useEffect(() => {
    const read = coalesce(() => {
      const id = current.current;
      if (id == null) return;
      void listedLink(id).then((found) => {
        if (found && current.current === id) setProbe(found);
      });
    }, READ_EVERY_MS);
    const stopProgress = onJobProgress((report) => {
      if (report.id === current.current) read();
    });
    const stopFinished = onJobFinished(({ id, result }) => {
      if (id === current.current) return finish(id, result);
      ended.current.set(id, result);
      if (ended.current.size > ENDED_KEPT) ended.current.delete(ended.current.keys().next().value as number);
    });
    return () => {
      stopProgress();
      stopFinished();
    };
  }, [finish]);

  const stop = useCallback(() => {
    if (current.current != null) void cancelJob(current.current);
  }, []);

  /** Stops the check and forgets what it found. */
  const clear = useCallback(() => {
    stop();
    current.current = null;
    setProbe(null);
    setRunning(false);
    setError(null);
  }, [stop]);

  /** Checks `url`, stopping the check before. */
  const check = useCallback(
    async (url: string) => {
      clear();
      setRunning(true);
      try {
        const id = await listLink(url);
        current.current = id;
        const result = ended.current.get(id);
        if (result) finish(id, result);
      } catch (e) {
        setRunning(false);
        setError(String(e));
      }
    },
    [clear, finish],
  );

  return { probe, running, error, check, stop, clear };
}

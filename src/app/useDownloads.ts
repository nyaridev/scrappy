// useDownloads: the kept downloads from Rust, with each one's progress from job events, and removing them from the
// list. One store for the whole window (Download and Grab both read it), listening from the first use until the app
// closes. Events come in bursts (a grab of a thousand files sends a thousand), so progress is applied in batches and
// the list is read again at most twice a second. `useDownload` reads one download, so a row redraws only when its
// own download changes. A hidden view keeps the copy it last showed and catches up when it shows. Downloads the
// extension sends show up when their first progress arrives.

import { useCallback, useRef, useSyncExternalStore } from "react";
import {
  cancelJob,
  clearDownloads,
  downloads,
  onJobFinished,
  onJobProgress,
  removeDownload,
  startDownload,
} from "../lib/backend";
import { coalesce } from "../lib/coalesce";
import type { DownloadEntry } from "../lib/types/DownloadEntry";
import type { DownloadRequest } from "../lib/types/DownloadRequest";
import type { DownloadSource } from "../lib/types/DownloadSource";
import type { JobProgress } from "../lib/types/JobProgress";
import { useViewActive } from "./viewShown";

// ── Constants ─────────────────────────────
const PROGRESS_MS = 200;
const RELOAD_MS = 500;

// ── Types ─────────────────────────────────
interface Snapshot {
  /** Newest first. */
  entries: DownloadEntry[];
  byId: Map<number, DownloadEntry>;
  progress: Record<number, JobProgress>;
}

// ── State ─────────────────────────────────
let snapshot: Snapshot = { entries: [], byId: new Map(), progress: {} };
let pending: JobProgress[] = [];
let listening = false;
const listeners = new Set<() => void>();
/** Job ids the list was read again for already; tool installs report progress too and aren't downloads. */
const lookedUp = new Set<number>();

// ── Private helpers ───────────────────────
function publish(next: Partial<Snapshot>) {
  snapshot = { ...snapshot, ...next };
  for (const listener of listeners) listener();
}

/** Takes a list from Rust (oldest first). */
function keep(fresh: DownloadEntry[]) {
  publish({ entries: [...fresh].reverse(), byId: new Map(fresh.map((entry) => [entry.id, entry])) });
}

const reload = coalesce(() => void downloads().then(keep), RELOAD_MS);

const flush = coalesce(() => {
  if (pending.length === 0) return;
  const progress = { ...snapshot.progress };
  for (const report of pending) progress[report.id] = report;
  pending = [];
  publish({ progress });
}, PROGRESS_MS);

function listen() {
  if (listening) return;
  listening = true;
  reload();
  onJobProgress((report) => {
    if (!snapshot.byId.has(report.id) && !lookedUp.has(report.id)) {
      lookedUp.add(report.id);
      reload();
    }
    pending.push(report);
    flush();
  });
  onJobFinished(reload);
}

function subscribe(listener: () => void) {
  listen();
  listeners.add(listener);
  return () => void listeners.delete(listener);
}

/** Part of the store, frozen while the view is hidden. `select` must be stable and return the store's own objects. */
function useStore<T>(select: (store: Snapshot) => T): T {
  const active = useViewActive();
  const seen = useRef<{ value: T } | null>(null);
  const read = useCallback(() => {
    if (active || !seen.current) seen.current = { value: select(snapshot) };
    return seen.current.value;
  }, [active, select]);
  return useSyncExternalStore(subscribe, read);
}

const whole = (store: Snapshot) => store;

/** The actions; the same functions every render, so lists can skip redrawing. */
const ACTIONS = {
  start: async (request: DownloadRequest) =>
    keep([...[...snapshot.entries].reverse(), ...(await startDownload(request))]),
  cancel: (id: number) => void cancelJob(id),
  remove: (id: number) => void removeDownload(id).then(keep),
  /** Removes the finished rows, or only those from `source`. */
  clear: (source: DownloadSource | null = null) => void clearDownloads(source).then(keep),
};

// ── Public API ────────────────────────────
/** The downloads, newest first, their progress by id, and the actions. */
export function useDownloads() {
  const { entries, progress } = useStore(whole);
  return { entries, progress, ...ACTIONS };
}

/** One download and its last progress report, by job id; both `undefined` when there's none. It redraws only when
 * that download moves, so it never freezes: a table of a thousand of these would all redraw on every view switch. */
export function useDownload(id: number | null | undefined): { entry?: DownloadEntry; report?: JobProgress } {
  const entry = useSyncExternalStore(subscribe, () => (id == null ? undefined : snapshot.byId.get(id)));
  const report = useSyncExternalStore(subscribe, () => (id == null ? undefined : snapshot.progress[id]));
  return { entry, report };
}

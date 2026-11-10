// DownloadQueue: this session's downloads, newest first: thumbnail, title, preset, where each one is (files done of
// the total for a playlist or profile), a progress bar while it runs, and a stop button (queued or running) or a
// show-in-folder button (done).

import { Icon, IconButton, Image, ProgressBar, Tooltip } from "iro-ui";
import { AlertCircleIcon, CheckCircleIcon, PlayIcon, StopIcon } from "iro-ui/icons";
import { ShowInFolderButton } from "../../components/ShowInFolderButton";
import { formatBytes, formatCount } from "../../lib/format";
import type { DownloadEntry } from "../../lib/types/DownloadEntry";
import type { JobProgress } from "../../lib/types/JobProgress";

// ── Private helpers ───────────────────────
/** The entry's state, counting a queued entry as running once its job reports progress (the list refreshes only
 * when a job ends). */
function stateOf(entry: DownloadEntry, progress: JobProgress | undefined): DownloadEntry["state"] {
  return entry.state === "queued" && progress && progress.phase !== "queued" ? "running" : entry.state;
}

/** What a finished playlist or profile saved: `12 files · 3 already saved · Social/bluesky/bsky.app`. */
function savedText(entry: DownloadEntry): string {
  if (entry.files == null) return entry.file ?? "Done";
  const parts = [formatCount(entry.files, "file"), entry.skipped ? `${entry.skipped} already saved` : null, entry.file];
  return parts.filter(Boolean).join(" · ");
}

function detail(entry: DownloadEntry, progress: JobProgress | undefined): string {
  switch (stateOf(entry, progress)) {
    case "done":
      return savedText(entry);
    case "failed":
      return entry.error ?? "Failed";
    case "cancelled":
      return "Stopped";
    case "queued":
      return "Queued";
  }
  if (!progress || progress.phase === "queued") return "Starting";
  if (progress.phase === "items") {
    const done =
      progress.total != null
        ? `${progress.done} of ${formatCount(progress.total, "file")}`
        : formatCount(progress.done, "file");
    return progress.message ? `${done} · ${progress.message}` : done;
  }
  if (progress.phase === "encode") {
    return progress.total ? `Converting ${Math.round((progress.done / progress.total) * 100)}%` : "Converting";
  }
  return progress.total
    ? `${formatBytes(progress.done)} of ${formatBytes(progress.total)}`
    : formatBytes(progress.done);
}

// ── Public API ────────────────────────────
/** The download list. */
export function DownloadQueue({
  entries,
  progress,
  onCancel,
}: {
  entries: DownloadEntry[];
  progress: Record<number, JobProgress>;
  onCancel: (id: number) => void;
}) {
  return (
    <ul className="queue" aria-label="Downloads">
      {entries.map((entry) => {
        const report = progress[entry.id];
        const state = stateOf(entry, report);
        const fraction = report?.total && report.phase !== "queued" ? report.done / report.total : null;
        const text = detail(entry, report);
        const finished = state === "done" || state === "failed";
        return (
          <li key={entry.id} className="queue-row" data-state={state}>
            {entry.thumbnail ? (
              <Image className="queue-thumbnail" src={entry.thumbnail} alt="" ratio={16 / 9} />
            ) : (
              <span className="queue-thumbnail queue-thumbnail-empty">
                <Icon icon={PlayIcon} />
              </span>
            )}
            <Tooltip content={entry.url}>
              <span className="queue-title">{entry.title}</span>
            </Tooltip>
            <span className="queue-preset">{entry.preset}</span>
            <Tooltip content={text} disabled={!finished}>
              <span className={finished ? "queue-detail selectable" : "queue-detail"}>
                {state === "done" && <Icon icon={CheckCircleIcon} />}
                {state === "failed" && <Icon icon={AlertCircleIcon} />}
                <span className="queue-detail-text">{text}</span>
              </span>
            </Tooltip>
            <span className="queue-actions">
              {(state === "queued" || state === "running") && (
                <IconButton size="sm" icon={<Icon icon={StopIcon} />} label="Stop" onClick={() => onCancel(entry.id)} />
              )}
              {state === "done" && entry.file && <ShowInFolderButton path={entry.file} />}
            </span>
            {state === "running" && (
              <ProgressBar className="queue-progress" size="sm" label={`${entry.title} progress`} value={fraction} />
            )}
          </li>
        );
      })}
    </ul>
  );
}

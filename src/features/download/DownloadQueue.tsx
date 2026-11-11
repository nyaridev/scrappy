// DownloadQueue: the kept downloads, newest first: thumbnail, title, preset, where each one is (files done of the
// total for a playlist or profile), a progress bar while it runs, and a stop button (queued or running), or a
// show-in-folder button (done) and a remove button (finished). Right-click: the same, plus Open and Copy link.

import { ContextMenu, Icon, IconButton, Image, type MenuEntry, ProgressBar, Tooltip } from "iro-ui";
import { AlertCircleIcon, CheckCircleIcon, CloseIcon, PlayIcon, StopIcon } from "iro-ui/icons";
import { memo, useEffect, useRef, useState } from "react";
import { copyLinkItem, fileMenu } from "../../components/fileMenu";
import { ShowInFolderButton } from "../../components/ShowInFolderButton";
import { formatBytes, formatCount } from "../../lib/format";
import type { DownloadEntry } from "../../lib/types/DownloadEntry";
import type { JobProgress } from "../../lib/types/JobProgress";

// ── Constants ─────────────────────────────
/** Rows drawn at first, and added each time the end of the list comes near: a grab can add a thousand. */
const PAGE = 100;

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

/** Same download, same look: a reload brings every row as a new object, so rows compare what they show. */
function sameEntry(a: DownloadEntry, b: DownloadEntry): boolean {
  return (
    a.id === b.id &&
    a.state === b.state &&
    a.title === b.title &&
    a.preset === b.preset &&
    a.file === b.file &&
    a.files === b.files &&
    a.skipped === b.skipped &&
    a.error === b.error
  );
}

interface RowProps {
  entry: DownloadEntry;
  report: JobProgress | undefined;
  onCancel: (id: number) => void;
  onRemove: (id: number) => void;
}

/** One download. Drawn again only when it or its progress changes: a list of a thousand rows gets a progress batch
 * five times a second. The callbacks aren't compared, since they only pass the id on. */
const DownloadRow = memo(
  function DownloadRow({ entry, report, onCancel, onRemove }: RowProps) {
    const state = stateOf(entry, report);
    const fraction = report?.total && report.phase !== "queued" ? report.done / report.total : null;
    const text = detail(entry, report);
    const finished = state === "done" || state === "failed";
    const stoppable = state === "queued" || state === "running";
    const menu: MenuEntry[] = [
      ...(state === "done" && entry.file ? fileMenu(entry.file) : []),
      copyLinkItem(entry.url),
      ...(stoppable ? [{ label: "Stop", icon: <Icon icon={StopIcon} />, onSelect: () => onCancel(entry.id) }] : []),
      ...(stoppable ? [] : [{ label: "Remove", icon: <Icon icon={CloseIcon} />, onSelect: () => onRemove(entry.id) }]),
    ];
    return (
      <ContextMenu items={menu}>
        <li className="queue-row" data-state={state}>
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
            {stoppable && (
              <IconButton size="sm" icon={<Icon icon={StopIcon} />} label="Stop" onClick={() => onCancel(entry.id)} />
            )}
            {state === "done" && entry.file && <ShowInFolderButton path={entry.file} />}
            {!stoppable && (
              <IconButton
                size="sm"
                icon={<Icon icon={CloseIcon} />}
                label="Remove"
                onClick={() => onRemove(entry.id)}
              />
            )}
          </span>
          {state === "running" && (
            <ProgressBar className="queue-progress" size="sm" label={`${entry.title} progress`} value={fraction} />
          )}
        </li>
      </ContextMenu>
    );
  },
  (before, after) => before.report === after.report && sameEntry(before.entry, after.entry),
);

// ── Public API ────────────────────────────
/** The download list; skipped when the list, progress and actions are the same (switching views). */
export const DownloadQueue = memo(function DownloadQueue({
  entries,
  progress,
  onCancel,
  onRemove,
}: {
  entries: DownloadEntry[];
  progress: Record<number, JobProgress>;
  onCancel: (id: number) => void;
  onRemove: (id: number) => void;
}) {
  const [limit, setLimit] = useState(PAGE);
  const end = useRef<HTMLLIElement>(null);
  const more = entries.length > limit;

  useEffect(() => {
    const target = end.current;
    if (!more || !target) return;
    const observer = new IntersectionObserver((seen) => seen[0]?.isIntersecting && setLimit((n) => n + PAGE), {
      rootMargin: "600px",
    });
    observer.observe(target);
    return () => observer.disconnect();
  }, [more]);

  return (
    <ul className="queue" aria-label="Downloads">
      {entries.slice(0, limit).map((entry) => (
        <DownloadRow key={entry.id} entry={entry} report={progress[entry.id]} onCancel={onCancel} onRemove={onRemove} />
      ))}
      {more && <li ref={end} className="queue-end" aria-hidden />}
    </ul>
  );
});

// DownloadQueue: the kept downloads, newest first: thumbnail, title, preset (for files kept as they are, their types:
// `MP4`, or `JPG, MP4, PNG`), where each one is (files done of the
// total for a playlist or profile), a progress bar while it runs, and a stop button (queued or running), or a
// show-in-folder button (done, or failed where it had a folder) and a remove button (finished). A done row with a
// note (kept without audio) shows it in orange. Right-click: the same, plus Open and Copy link. A long list is shown
// a page at a time.

import { ContextMenu, Icon, IconButton, Image, type MenuEntry, Pagination, ProgressBar, Tooltip } from "iro-ui";
import {
  AlertCircleIcon,
  CheckCircleIcon,
  CloseIcon,
  GlobeIcon,
  type IconData,
  PlayIcon,
  SearchIcon,
  StopIcon,
  TargetIcon,
} from "iro-ui/icons";
import { memo, useRef, useState } from "react";
import { formatBytes, formatCount } from "../lib/format";
import type { DownloadEntry } from "../lib/types/DownloadEntry";
import type { DownloadSource } from "../lib/types/DownloadSource";
import type { JobProgress } from "../lib/types/JobProgress";
import { copyLinkItem, fileMenu } from "./fileMenu";
import { ShowInFolderButton } from "./ShowInFolderButton";

// ── Constants ─────────────────────────────
/** Rows on one page: a grab can add a thousand. */
const PAGE_SIZE = 50;
/** A row without a thumbnail shows the icon of the view it came from. */
const SOURCE_ICONS: Record<DownloadSource, IconData> = {
  download: PlayIcon,
  extension: PlayIcon,
  grab: TargetIcon,
  scrape: GlobeIcon,
  hunt: SearchIcon,
};

// ── Private helpers ───────────────────────
/** The entry's state, counting a queued entry as running once its job reports progress (the list refreshes only
 * when a job ends). */
function stateOf(entry: DownloadEntry, progress: JobProgress | undefined): DownloadEntry["state"] {
  return entry.state === "queued" && progress && progress.phase !== "queued" ? "running" : entry.state;
}

/** What a finished download saved: `48.3 MB · Videos/…`, or for a playlist or profile
 * `12 files · 1.2 GB · 3 already saved · Social/bluesky/bsky.app`. */
function savedText(entry: DownloadEntry): string {
  const size = entry.size ? formatBytes(entry.size) : null;
  if (entry.files == null) return [size, entry.file ?? "Done"].filter(Boolean).join(" · ");
  const skipped = entry.skipped ? `${entry.skipped} already saved` : null;
  return [formatCount(entry.files, "file"), size, skipped, entry.file].filter(Boolean).join(" · ");
}

function detail(entry: DownloadEntry, progress: JobProgress | undefined): string {
  switch (stateOf(entry, progress)) {
    case "done":
      return [entry.error, savedText(entry)].filter(Boolean).join(" · ");
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
    a.size === b.size &&
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
    const noted = state === "done" && entry.error != null;
    const menu: MenuEntry[] = [
      ...(finished && entry.file ? fileMenu(entry.file) : []),
      ...(entry.url ? [copyLinkItem(entry.url)] : []),
      ...(stoppable ? [{ label: "Stop", icon: <Icon icon={StopIcon} />, onSelect: () => onCancel(entry.id) }] : []),
      ...(stoppable ? [] : [{ label: "Remove", icon: <Icon icon={CloseIcon} />, onSelect: () => onRemove(entry.id) }]),
    ];
    return (
      <ContextMenu items={menu}>
        <li className="queue-row" data-state={state} data-noted={noted || undefined}>
          {entry.thumbnail ? (
            <Image className="queue-thumbnail" src={entry.thumbnail} alt="" ratio={16 / 9} />
          ) : (
            <span className="queue-thumbnail queue-thumbnail-empty">
              <Icon icon={SOURCE_ICONS[entry.source]} />
            </span>
          )}
          <Tooltip content={entry.url} disabled={!entry.url}>
            <span className="queue-title">{entry.title}</span>
          </Tooltip>
          {/* Files kept as they are name their types in the preset's place. */}
          <span className="queue-preset">{entry.types.length > 0 ? entry.types.join(", ") : entry.preset}</span>
          <Tooltip content={text} disabled={!finished}>
            <span className={finished ? "queue-detail selectable" : "queue-detail"}>
              {state === "done" && <Icon icon={noted ? AlertCircleIcon : CheckCircleIcon} />}
              {state === "failed" && <Icon icon={AlertCircleIcon} />}
              <span className="queue-detail-text">{text}</span>
            </span>
          </Tooltip>
          <span className="queue-actions">
            {stoppable && (
              <IconButton size="sm" icon={<Icon icon={StopIcon} />} label="Stop" onClick={() => onCancel(entry.id)} />
            )}
            {finished && entry.file && <ShowInFolderButton path={entry.file} />}
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
  const [pageNumber, setPageNumber] = useState(1);
  const top = useRef<HTMLUListElement>(null);
  // Rows leave as they're removed or cleared: a page past the end shows the last one.
  const current = Math.min(pageNumber, Math.max(1, Math.ceil(entries.length / PAGE_SIZE)));
  const rows = entries.slice((current - 1) * PAGE_SIZE, current * PAGE_SIZE);

  function turnPage(next: number) {
    setPageNumber(next);
    top.current?.scrollIntoView({ block: "start" });
  }

  return (
    <>
      <ul ref={top} className="queue" aria-label="Downloads">
        {rows.map((entry) => (
          <DownloadRow
            key={entry.id}
            entry={entry}
            report={progress[entry.id]}
            onCancel={onCancel}
            onRemove={onRemove}
          />
        ))}
      </ul>
      {entries.length > PAGE_SIZE && (
        <Pagination page={current} pageSize={PAGE_SIZE} total={entries.length} onPageChange={turnPage} />
      )}
    </>
  );
});

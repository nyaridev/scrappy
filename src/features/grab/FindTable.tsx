// FindTable: a page's finds with their type, name, size and, once picked, how the download is going. Rows can be
// picked and sorted.

import { Badge, DataTable, type DataTableColumn, Icon, ProgressBar, Tooltip } from "iro-ui";
import { AlertCircleIcon, CheckCircleIcon } from "iro-ui/icons";
import { formatBytes } from "../../lib/format";
import type { DownloadEntry } from "../../lib/types/DownloadEntry";
import type { Find } from "../../lib/types/Find";
import type { JobProgress } from "../../lib/types/JobProgress";
import type { Picked } from "../../lib/types/Picked";
import { FIND_KINDS } from "./findKinds";

// ── Constants ─────────────────────────────
const KIND_ORDER = Object.keys(FIND_KINDS);

// ── Types ─────────────────────────────────
interface Props {
  finds: Find[];
  picked: Record<string, Picked>;
  entries: DownloadEntry[];
  progress: Record<number, JobProgress>;
  selected: string[];
  onSelectedChange: (urls: string[]) => void;
}

// ── Private helpers ───────────────────────
function Status({
  find,
  pick,
  entry,
  report,
}: {
  find: Find;
  pick?: Picked;
  entry?: DownloadEntry;
  report?: JobProgress;
}) {
  if (!pick) return null;
  const error = pick.error ?? (entry?.state === "failed" ? (entry.error ?? "Failed") : null);
  if (error) {
    return (
      <Tooltip content={error}>
        <span className="find-status selectable" data-state="failed">
          <Icon icon={AlertCircleIcon} />
          Failed
        </span>
      </Tooltip>
    );
  }
  if (entry?.state === "done") {
    return (
      <span className="find-status" data-state="done">
        <Icon icon={CheckCircleIcon} />
        Saved
      </span>
    );
  }
  if (entry?.state === "cancelled") return <span className="find-status">Stopped</span>;
  if (!report || report.phase === "queued") return <span className="find-status">Queued</span>;
  const fraction = report.total ? report.done / report.total : null;
  return <ProgressBar size="sm" label={`${find.name} progress`} value={fraction} />;
}

// ── Public API ────────────────────────────
/** The finds table. */
export function FindTable({ finds, picked, entries, progress, selected, onSelectedChange }: Props) {
  const columns: DataTableColumn<Find>[] = [
    {
      id: "kind",
      header: "Type",
      width: "calc(var(--iro-space-6) * 5)",
      sortValue: (find) => KIND_ORDER.indexOf(find.kind),
      cell: (find) => {
        const look = FIND_KINDS[find.kind];
        return (
          <Badge tone={look.tone} icon={<Icon icon={look.icon} />}>
            {look.label}
          </Badge>
        );
      },
    },
    {
      id: "name",
      header: "Name",
      sortValue: (find) => find.name.toLowerCase(),
      cell: (find) => (
        <Tooltip content={find.url}>
          <span className="find-name">{find.name}</span>
        </Tooltip>
      ),
    },
    {
      id: "size",
      header: "Size",
      align: "end",
      width: "calc(var(--iro-space-6) * 4)",
      sortValue: (find) => find.size ?? -1,
      cell: (find) => (find.size != null ? formatBytes(find.size) : ""),
    },
    {
      id: "status",
      header: "Download",
      width: "calc(var(--iro-space-6) * 6)",
      cell: (find) => {
        const pick = picked[find.url];
        const entry = pick?.id != null ? entries.find((e) => e.id === pick.id) : undefined;
        const report = pick?.id != null ? progress[pick.id] : undefined;
        return <Status find={find} pick={pick} entry={entry} report={report} />;
      },
    },
  ];
  return (
    <DataTable
      label="Found on the page"
      className="find-table"
      size="sm"
      columns={columns}
      rows={finds}
      rowKey={(find) => find.url}
      rowLabel={(find) => find.name}
      selectable
      selected={selected}
      onSelectedChange={onSelectedChange}
      empty="Nothing of this type."
    />
  );
}

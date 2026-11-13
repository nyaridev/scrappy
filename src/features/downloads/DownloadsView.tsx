// DownloadsView: every download in one list, newest first (ADR-0018): from the Download view, Grab, Scrape, Hunt and
// the extension, filtered by where it came from, with Clear all for the finished rows the filter shows.

import { Button, EmptyState, Icon, Segmented } from "iro-ui";
import { TrayInIcon } from "iro-ui/icons";
import { useMemo, useState } from "react";
import { useDownloads } from "../../app/useDownloads";
import { DownloadQueue } from "../../components/DownloadQueue";
import type { DownloadSource } from "../../lib/types/DownloadSource";
import "./downloads.css";

// ── Constants ─────────────────────────────
const ALL = "all";
const SOURCES: { value: DownloadSource; label: string }[] = [
  { value: "download", label: "Download" },
  { value: "grab", label: "Grab" },
  { value: "scrape", label: "Scrape" },
  { value: "hunt", label: "Hunt" },
  { value: "extension", label: "Extension" },
];
const FILTERS = [{ value: ALL, label: "All" }, ...SOURCES];

// ── Public API ────────────────────────────
/** Downloads */
export function DownloadsView() {
  const [filter, setFilter] = useState(ALL);
  const { entries, progress, cancel, remove, clear } = useDownloads();
  const source = SOURCES.find((each) => each.value === filter);
  const shown = useMemo(() => (source ? entries.filter((e) => e.source === source.value) : entries), [entries, source]);
  const finished = shown.some((entry) => entry.state !== "queued" && entry.state !== "running");

  return (
    <div className="downloads-page">
      <div className="downloads-bar">
        <Segmented
          size="sm"
          aria-label="Show downloads from"
          options={FILTERS}
          value={filter}
          onValueChange={setFilter}
        />
        <Button size="sm" variant="ghost" disabled={!finished} onClick={() => clear(source?.value ?? null)}>
          Clear all
        </Button>
      </div>
      {shown.length > 0 ? (
        <DownloadQueue key={filter} entries={shown} progress={progress} onCancel={cancel} onRemove={remove} />
      ) : (
        <EmptyState
          icon={<Icon icon={TrayInIcon} size={32} />}
          title={source ? `No downloads from ${source.label}` : "No downloads"}
        />
      )}
    </div>
  );
}

// HuntProgress: a hunt's progress under the form: its state, images saved, sources done and failed, duplicates
// removed (ADR-0027), the bar for the whole hunt, its folder, a duplicate scan once done, and a row for each source
// (ADR-0024).

import { Button, Icon, Panel, ProgressBar } from "iro-ui";
import { DuplicateIcon } from "iro-ui/icons";
import { ShowInFolderButton } from "../../components/ShowInFolderButton";
import { formatCount } from "../../lib/format";
import { SourceRow } from "./SourceRow";
import type { HuntState } from "./useHunt";

// ── Constants ─────────────────────────────
const STATE_WORDS = { running: "Hunting", done: "Finished", stopped: "Stopped", failed: "Failed" } as const;

// ── Public API ────────────────────────────
/** The panel, for a hunt that started. */
export function HuntProgress({
  hunt,
  onDuplicates,
}: {
  hunt: Exclude<HuntState, { state: "idle" }>;
  onDuplicates: () => void;
}) {
  const { sources, removing, removed } = hunt.status;
  const done = sources.filter((one) => one.state === "done").length;
  const failed = sources.filter((one) => one.state === "failed").length;
  const saved = sources.reduce((sum, one) => sum + one.saved, 0);
  const progress = hunt.state === "failed" ? null : hunt.progress;
  const facts = [
    removing ? "Removing duplicates" : STATE_WORDS[hunt.state],
    `${formatCount(saved, "image")} saved`,
    hunt.state === "running"
      ? `${done} of ${formatCount(sources.length, "source")} done`
      : `${formatCount(done, "source")} done`,
    failed > 0 ? `${failed} failed` : null,
    removed != null ? `${formatCount(removed, "duplicate")} removed` : null,
  ];

  return (
    <Panel as="section" className="hunt-status" aria-label="Hunt progress">
      {hunt.state === "running" && (
        <ProgressBar
          label={removing ? "Removing duplicates" : "Images saved"}
          value={progress?.total ? progress.done / progress.total : null}
        />
      )}
      <div className="scrape-counts">
        <span>{facts.filter(Boolean).join(" · ")}</span>
        {hunt.state !== "failed" && (
          <>
            <span className="scrape-folder selectable">{hunt.folder}</span>
            <ShowInFolderButton path={hunt.folder} />
          </>
        )}
        {hunt.state === "done" && saved > 1 && (
          <Button size="sm" icon={<Icon icon={DuplicateIcon} />} onClick={onDuplicates}>
            Find duplicates
          </Button>
        )}
      </div>
      {sources.length > 0 && (
        <ul className="hunt-sources">
          {sources.map((source) => (
            <SourceRow key={source.name} source={source} />
          ))}
        </ul>
      )}
    </Panel>
  );
}

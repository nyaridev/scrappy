// DuplicatesPanel: the duplicate finder (the Gallery's, and Hunt's for its folder): pick how alike copies must be,
// scan the library or one folder, look through the groups (each one's best copy marked, the rest picked) and move the picked copies to the trash.

import { AlertDialog, Button, EmptyState, Icon, IconButton, ProgressBar, Segmented, toast } from "iro-ui";
import { ArrowLeftIcon, DuplicateIcon, TrashIcon } from "iro-ui/icons";
import { useState } from "react";
import { formatCount } from "../lib/format";
import type { Strictness } from "../lib/types/Strictness";
import { DuplicateGroup } from "./DuplicateGroup";
import { StopButton } from "./StopButton";
import { useDuplicates } from "./useDuplicates";

// ── Constants ─────────────────────────────
const STRICTNESS: { value: Strictness; label: string }[] = [
  { value: "strict", label: "Strict" },
  { value: "normal", label: "Normal" },
  { value: "loose", label: "Loose" },
];

// ── Public API ────────────────────────────
/** The panel, for the whole library or `folder`; `onClose` goes back, with `changed` when files moved to the trash. */
export function DuplicatesPanel({ folder, onClose }: { folder?: string; onClose: (changed: boolean) => void }) {
  const { result, running, progress, picked, start, stop, toggle, trashPicked } = useDuplicates(folder);
  const [strictness, setStrictness] = useState<Strictness>("normal");
  const [asking, setAsking] = useState(false);
  const [changed, setChanged] = useState(false);
  const copies = formatCount(picked.size, "file");

  async function moveToTrash() {
    try {
      const moved = await trashPicked();
      setChanged(true);
      toast({ tone: "success", title: `Moved ${formatCount(moved, "file")} to the trash` });
    } catch (e) {
      setChanged(true);
      toast({ tone: "error", title: "Couldn't move every file", description: String(e) });
    }
  }

  return (
    <section className="duplicates" aria-label="Duplicates">
      <div className="duplicates-bar">
        <IconButton icon={<Icon icon={ArrowLeftIcon} />} label="Back" onClick={() => onClose(changed)} />
        <span className="duplicates-title">Duplicates</span>
        <Segmented
          size="sm"
          aria-label="How alike"
          options={STRICTNESS}
          value={strictness}
          onValueChange={setStrictness}
          disabled={running}
        />
        {running ? (
          <StopButton onStop={stop} />
        ) : (
          <Button variant="primary" icon={<Icon icon={DuplicateIcon} />} onClick={() => void start(strictness)}>
            Find duplicates
          </Button>
        )}
      </div>

      {running && (
        <div className="duplicates-progress">
          <ProgressBar label="Scan progress" value={progress?.total ? progress.done / progress.total : null} />
          <span className="duplicates-note">
            {progress?.total ? `${progress.done} of ${formatCount(progress.total, "picture")}` : "Starting"}
            {progress?.message ? ` · ${progress.message}` : ""}
          </span>
        </div>
      )}
      {!running && result.state === "failed" && (
        <EmptyState tone="warning" title="The scan failed">
          {result.message}
        </EmptyState>
      )}
      {!running && result.state === "done" && result.groups.length === 0 && (
        <EmptyState title={changed ? "No duplicates left" : "No duplicates found"} />
      )}
      {!running && result.state === "done" && result.groups.length > 0 && (
        <>
          <div className="duplicates-summary">
            <span className="duplicates-note">
              {formatCount(result.groups.length, "group")} · {copies} picked
            </span>
            <Button
              variant="danger"
              icon={<Icon icon={TrashIcon} />}
              disabled={picked.size === 0}
              onClick={() => setAsking(true)}
            >
              Move {picked.size} to trash
            </Button>
          </div>
          <div className="duplicates-groups">
            {result.groups.map((group) => (
              <DuplicateGroup key={group[0]?.path} group={group} picked={picked} onPick={toggle} />
            ))}
          </div>
        </>
      )}

      <AlertDialog
        open={asking}
        onOpenChange={setAsking}
        title={`Move ${copies} to the trash?`}
        description="They leave the library. You can restore them from the system trash."
        confirmLabel="Move to trash"
        onConfirm={moveToTrash}
      />
    </section>
  );
}

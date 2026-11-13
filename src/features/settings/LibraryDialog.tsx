// LibraryDialog: a folder to use instead of the library, or to move the library to (ADR-0039). The folder is typed or
// picked and Rust checks it, as at first-run setup; a move then runs as a job, with its progress and a Stop button.

import { Button, Dialog, Field, Icon, IconButton, Notice, ProgressBar, TextField } from "iro-ui";
import { FolderIcon, FolderOpenIcon } from "iro-ui/icons";
import { useEffect, useState } from "react";
import {
  cancelJob,
  checkFolder,
  libraryStatus,
  moveLibrary,
  onJobFinished,
  onJobProgress,
  pickFolder,
  setupLibrary,
} from "../../lib/backend";
import { formatBytes } from "../../lib/format";
import type { FolderCheck } from "../../lib/types/FolderCheck";
import type { JobProgress } from "../../lib/types/JobProgress";
import type { LibraryStatus } from "../../lib/types/LibraryStatus";

// ── Constants ─────────────────────────────
/** How long typing pauses before Rust checks the path. */
const CHECK_DELAY_MS = 300;

// ── Types ─────────────────────────────────
/** Use another folder from now on, or take this library there. */
export type LibraryChange = "switch" | "move";

// ── Private helpers ───────────────────────
/** The main button's label for what Rust found. */
function actionFor(change: LibraryChange, found: FolderCheck | null): string {
  if (change === "move") return "Move library";
  if (found?.library) return "Use this library";
  if (found?.exists && !found.empty) return "Use this folder";
  return "Create library";
}

// ── Public API ────────────────────────────
/** The dialog for `change`; `onChanged` gets the library's new status. */
export function LibraryDialog({
  change,
  root,
  onClose,
  onChanged,
}: {
  change: LibraryChange;
  root: string;
  onClose: () => void;
  onChanged: (status: LibraryStatus) => void;
}) {
  const [path, setPath] = useState("");
  const [found, setFound] = useState<FolderCheck | null>(null);
  const [pathError, setPathError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [job, setJob] = useState<number | null>(null);
  const [progress, setProgress] = useState<JobProgress | null>(null);

  // A newer path drops the answer for an older one.
  useEffect(() => {
    setFound(null);
    setPathError(null);
    if (!path.trim()) return;
    let current = true;
    const timer = setTimeout(() => {
      checkFolder(path).then(
        (check) => current && setFound(check),
        (e) => current && setPathError(String(e)),
      );
    }, CHECK_DELAY_MS);
    return () => {
      current = false;
      clearTimeout(timer);
    };
  }, [path]);

  // The move's reports, until it ends.
  useEffect(() => {
    if (job === null) return;
    const stopProgress = onJobProgress((report) => report.id === job && setProgress(report));
    const stopFinished = onJobFinished(({ id, result }) => {
      if (id !== job) return;
      setJob(null);
      setProgress(null);
      setBusy(false);
      if (result.state === "done") void libraryStatus().then(onChanged);
      else setError(result.state === "failed" ? result.message : `Stopped. The library stays in ${root}.`);
    });
    return () => {
      stopProgress();
      stopFinished();
    };
  }, [job, root, onChanged]);

  // Rust refuses these too; saying so here spares a click.
  const full = change === "move" && found?.exists === true && !found.empty;
  const ready = Boolean(found?.writable) && !full && !busy;

  async function browse() {
    setError(null);
    try {
      const check = await pickFolder();
      if (check) setPath(check.path);
    } catch (e) {
      setError(String(e));
    }
  }

  async function confirm() {
    if (!ready) return;
    setBusy(true);
    setError(null);
    try {
      if (change === "move") setJob(await moveLibrary(path));
      else onChanged(await setupLibrary(path));
    } catch (e) {
      setError(String(e));
      setBusy(false);
    }
  }

  const running = job !== null;
  return (
    <Dialog
      open
      // A running move keeps the dialog: it's where its progress and Stop are.
      onOpenChange={(open) => !open && !running && onClose()}
      title={change === "move" ? "Move library" : "Use another folder"}
      description={change === "move" ? "Copies the library there, then uses it. The old folder stays." : undefined}
      footer={
        running ? (
          <Button onClick={() => void cancelJob(job)}>Stop</Button>
        ) : (
          <>
            <Button onClick={onClose}>Cancel</Button>
            <Button variant="primary" disabled={!ready} loading={busy} onClick={() => void confirm()}>
              {actionFor(change, found)}
            </Button>
          </>
        )
      }
    >
      <div className="settings-section">
        <Field
          label="Folder"
          hint={found && !found.exists ? "New folder." : undefined}
          error={
            pathError ??
            (found && !found.writable ? "Can't use this folder." : full ? "Choose an empty folder." : undefined)
          }
        >
          <TextField
            value={path}
            disabled={running}
            onChange={(event) => setPath(event.target.value)}
            onKeyDown={(event) => event.key === "Enter" && void confirm()}
            spellCheck={false}
            autoComplete="off"
            start={<Icon icon={FolderIcon} />}
            end={
              <IconButton icon={<Icon icon={FolderOpenIcon} />} label="Browse" onClick={browse} disabled={running} />
            }
          />
        </Field>
        {change === "switch" && found?.writable && found.exists && !found.library && !found.empty && (
          <Notice tone="warning" title="This folder isn't empty">
            Nothing in it is moved or deleted.
          </Notice>
        )}
        {running && (
          <div className="settings-row">
            <ProgressBar label="Copying the library" value={progress?.total ? progress.done / progress.total : null} />
            <p className="settings-hint">
              {progress?.total ? `${formatBytes(progress.done)} of ${formatBytes(progress.total)}` : "Starting"}
            </p>
          </div>
        )}
        {error && (
          <Notice tone="danger" title={change === "move" ? "Couldn't move the library" : "Couldn't use this folder"}>
            <span className="selectable">{error}</span>
          </Notice>
        )}
      </div>
    </Dialog>
  );
}

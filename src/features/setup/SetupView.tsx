// SetupView: the first-run screen, shown instead of the tabs until there's a library. A centered title over one card:
// the folder field (typed, or filled by the system picker; Rust checks it either way), a warning or error when one
// applies, and the button that fits.

import { Button, Field, Icon, IconButton, Notice, Panel, TextField } from "iro-ui";
import { FolderIcon, FolderOpenIcon, FolderPlusIcon } from "iro-ui/icons";
import { useEffect, useState } from "react";
import { checkFolder, pickFolder, setupLibrary } from "../../lib/backend";
import type { FolderCheck } from "../../lib/types/FolderCheck";
import type { LibraryStatus } from "../../lib/types/LibraryStatus";

// ── Constants ─────────────────────────────
/** How long typing pauses before Rust checks the path. */
const CHECK_DELAY_MS = 300;

// ── Private helpers ───────────────────────
/** The main button's label for what Rust found. */
function actionFor(found: FolderCheck | null): string {
  if (found?.library) return "Use this library";
  if (found?.exists && !found.empty) return "Use this folder";
  return "Create library";
}

// ── Public API ────────────────────────────
/** Picks or types the library folder, sets it up, then hands the new status up. */
export function SetupView({ status, onReady }: { status: LibraryStatus; onReady: (status: LibraryStatus) => void }) {
  const [path, setPath] = useState("");
  const [found, setFound] = useState<FolderCheck | null>(null);
  const [pathError, setPathError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

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

  const ready = Boolean(found?.writable) && !busy;

  async function browse() {
    setError(null);
    try {
      const check = await pickFolder();
      if (check) setPath(check.path);
    } catch (e) {
      setError(String(e));
    }
  }

  async function create() {
    if (!ready) return;
    setBusy(true);
    setError(null);
    try {
      onReady(await setupLibrary(path));
    } catch (e) {
      setError(String(e));
      setBusy(false);
    }
  }

  return (
    <div className="setup">
      <h1 id="setup-title" className="setup-title">
        Choose a library folder
      </h1>

      <Panel as="section" className="setup-card" aria-labelledby="setup-title">
        {status.state === "missing" && !path && (
          <Notice tone="warning" title="Library not found">
            {status.root}
          </Notice>
        )}

        <Field
          label="Folder"
          hint={found && !found.exists ? "New folder." : undefined}
          error={pathError ?? (found && !found.writable ? "Can't use this folder." : undefined)}
        >
          <TextField
            value={path}
            onChange={(event) => setPath(event.target.value)}
            onKeyDown={(event) => event.key === "Enter" && void create()}
            spellCheck={false}
            autoComplete="off"
            start={<Icon icon={FolderIcon} />}
            end={<IconButton icon={<Icon icon={FolderOpenIcon} />} label="Browse" onClick={browse} />}
          />
        </Field>

        {found?.writable && found.exists && !found.library && !found.empty && (
          <Notice tone="warning" title="This folder isn't empty">
            Nothing in it is moved or deleted.
          </Notice>
        )}
        {error && (
          <Notice tone="danger" title="Couldn't set up the library">
            {error}
          </Notice>
        )}

        <div className="setup-actions">
          <Button
            variant="primary"
            icon={<Icon icon={FolderPlusIcon} />}
            disabled={!ready}
            loading={busy}
            onClick={create}
          >
            {actionFor(found)}
          </Button>
        </div>
      </Panel>
    </div>
  );
}

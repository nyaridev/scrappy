// LibrarySettings: the library folder, and changing it: use another folder, or move this library there (ADR-0039).
// After a move, where the old folder still is, for the user to remove when they're sure.

import { Button, Icon, Notice, Panel, PropertyList, SectionTitle, toast } from "iro-ui";
import { FolderIcon, FolderMoveIcon, FolderOpenIcon } from "iro-ui/icons";
import { useCallback, useState } from "react";
import { useCached } from "../../app/useCached";
import { libraryStatus } from "../../lib/backend";
import type { LibraryStatus } from "../../lib/types/LibraryStatus";
import { type LibraryChange, LibraryDialog } from "./LibraryDialog";

// ── Public API ────────────────────────────
/** The Library section of Settings. */
export function LibrarySettings() {
  // Not taken from the shared settings, which were loaded before first-run setup saved the folder.
  const [library, setLibrary] = useCached("library", libraryStatus);
  const [change, setChange] = useState<LibraryChange | null>(null);
  const [left, setLeft] = useState<string | null>(null);
  const root = library && library.state !== "notSet" ? library.root : "";

  const changed = useCallback(
    (status: LibraryStatus) => {
      setLibrary(status);
      if (change === "move") setLeft(root);
      toast({ tone: "success", title: change === "move" ? "Library moved" : "Library changed" });
      setChange(null);
    },
    [change, root, setLibrary],
  );

  return (
    <Panel as="section" className="settings-section">
      <SectionTitle icon={<Icon icon={FolderIcon} />}>Library</SectionTitle>
      <PropertyList copyLabel="Copy" properties={[{ label: "Folder", value: root, mono: true, copy: root }]} />
      <div className="settings-actions">
        <Button size="sm" icon={<Icon icon={FolderOpenIcon} />} disabled={!root} onClick={() => setChange("switch")}>
          Use another folder
        </Button>
        <Button size="sm" icon={<Icon icon={FolderMoveIcon} />} disabled={!root} onClick={() => setChange("move")}>
          Move library
        </Button>
      </div>
      {left && (
        <Notice tone="info" title="The old folder is still there">
          <span className="selectable">{left}</span>
        </Notice>
      )}
      {change && <LibraryDialog change={change} root={root} onClose={() => setChange(null)} onChanged={changed} />}
    </Panel>
  );
}

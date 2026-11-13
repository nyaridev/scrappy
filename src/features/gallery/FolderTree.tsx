// FolderTree: the Gallery's sidebar: "All files", then the library's folders as a tree with their file counts. A click
// on a folder picks it and opens it; only its arrow closes it (ADR-0021). Right-click: Open folder, and on a
// downloaded folder (below the top level) Delete too (ADR-0022).

import { ContextMenu, Icon, type MenuEntry, toast } from "iro-ui";
import { ExternalLinkIcon, FolderIcon, FolderOpenIcon, ImagesIcon, TrashIcon } from "iro-ui/icons";
import { type ReactNode, useMemo, useState } from "react";
import { openFile } from "../../lib/backend";
import type { GalleryFolder } from "../../lib/types/GalleryFolder";
import { FolderRow } from "./FolderRow";

// ── Types ─────────────────────────────────
interface Props {
  folders: GalleryFolder[];
  /** The folder picked; `null` for all files. */
  selected: string | null;
  onSelect: (folder: string | null) => void;
  /** Asks to delete a downloaded folder; the view confirms first. */
  onDelete: (folder: string) => void;
}

// ── Private helpers ───────────────────────
function parentOf(path: string): string {
  const cut = path.lastIndexOf("/");
  return cut < 0 ? "" : path.slice(0, cut);
}

/** Opens a library folder in the system file manager. */
function openFolder(path: string) {
  openFile(path).catch((error) =>
    toast({ tone: "error", title: "Couldn't open the folder", description: String(error) }),
  );
}

// ── Public API ────────────────────────────
/** The folder sidebar. */
export function FolderTree({ folders, selected, onSelect, onDelete }: Props) {
  const [open, setOpen] = useState<string[]>([]);
  // Each folder's subfolders, by its path ("" for the top), in the order Rust sends them.
  const children = useMemo(() => {
    const all = new Map<string, GalleryFolder[]>();
    for (const folder of folders) {
      const parent = parentOf(folder.path);
      all.set(parent, [...(all.get(parent) ?? []), folder]);
    }
    return all;
  }, [folders]);

  const toggle = (path: string) => setOpen(open.includes(path) ? open.filter((one) => one !== path) : [...open, path]);

  const pick = (path: string) => {
    if (children.has(path) && !open.includes(path)) setOpen([...open, path]);
    onSelect(path);
  };

  const menu = (path: string, depth: number): MenuEntry[] => [
    { label: "Open folder", icon: <Icon icon={ExternalLinkIcon} />, onSelect: () => openFolder(path) },
    // The library's own folders (the top level) can't be deleted.
    ...(depth > 0
      ? [
          "separator" as const,
          { label: "Delete", icon: <Icon icon={TrashIcon} />, danger: true, onSelect: () => onDelete(path) },
        ]
      : []),
  ];

  const rows = (parent: string, depth: number): ReactNode[] =>
    (children.get(parent) ?? []).flatMap((folder) => {
      const inside = children.has(folder.path);
      const opened = inside && open.includes(folder.path);
      return [
        <ContextMenu key={folder.path} items={menu(folder.path, depth)}>
          <FolderRow
            name={folder.path.slice(parent ? parent.length + 1 : 0)}
            icon={<Icon icon={opened ? FolderOpenIcon : FolderIcon} />}
            depth={depth}
            files={folder.files}
            selected={selected === folder.path}
            open={inside ? opened : undefined}
            onPick={() => pick(folder.path)}
            onToggle={() => toggle(folder.path)}
          />
        </ContextMenu>,
        ...(opened ? rows(folder.path, depth + 1) : []),
      ];
    });

  return (
    <nav className="gallery-folders" aria-label="Folders">
      <FolderRow
        name="All files"
        icon={<Icon icon={ImagesIcon} />}
        depth={0}
        selected={selected == null}
        onPick={() => onSelect(null)}
      />
      {rows("", 0)}
    </nav>
  );
}

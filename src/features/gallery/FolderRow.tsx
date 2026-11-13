// FolderRow: one row of the Gallery's folder sidebar: an arrow that opens or closes the folder (when it has
// subfolders), then the row itself, which picks it: its icon, name and file count. Two buttons, unlike iro-ui's
// TreeRow, whose one button can only toggle a folder.

import { Icon } from "iro-ui";
import { ChevronRightIcon } from "iro-ui/icons";
import type { ComponentProps, CSSProperties, ReactNode } from "react";

// ── Types ─────────────────────────────────
interface Props extends Omit<ComponentProps<"div">, "onClick" | "onToggle"> {
  name: string;
  icon: ReactNode;
  depth: number;
  /** Files in it, subfolders' included; left out for "All files". */
  files?: number;
  selected: boolean;
  /** Present when it has subfolders: whether they show. */
  open?: boolean;
  onPick: () => void;
  onToggle?: () => void;
}

// ── Public API ────────────────────────────
/** A folder row. Extra props (a context menu's) go on the row. */
export function FolderRow({ name, icon, depth, files, selected, open, onPick, onToggle, style, ...rest }: Props) {
  return (
    <div
      {...rest}
      className="folder-row"
      data-selected={selected || undefined}
      style={{ ...style, "--depth": depth } as CSSProperties}
    >
      {open == null ? (
        <span className="folder-row-chevron" />
      ) : (
        <button
          type="button"
          className="folder-row-chevron"
          data-open={open || undefined}
          aria-expanded={open}
          aria-label={`${open ? "Close" : "Open"} ${name}`}
          onClick={onToggle}
        >
          <Icon icon={ChevronRightIcon} />
        </button>
      )}
      <button type="button" className="folder-row-main" aria-current={selected || undefined} onClick={onPick}>
        <span className="folder-row-icon">{icon}</span>
        <span className="folder-row-name">{name}</span>
        {files != null && <span className="folder-row-files">{files}</span>}
      </button>
    </div>
  );
}

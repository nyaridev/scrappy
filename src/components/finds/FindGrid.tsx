// FindGrid: a page of finds as tiles (`FindTile`); a click on a tile picks it, a shift-click picks a run of tiles.

import { memo } from "react";
import type { Find } from "../../lib/types/Find";
import type { Kept } from "../../lib/types/Kept";
import type { Picked } from "../../lib/types/Picked";
import { FindTile } from "./FindTile";
import { usePick } from "./usePick";

// ── Types ─────────────────────────────────
interface Props {
  finds: Find[];
  picked: Record<string, Picked>;
  /** How saving each find went, by link. */
  kept: { [url in string]?: Kept };
  selected: string[];
  onSelectedChange: (urls: string[]) => void;
}

// ── Public API ────────────────────────────
/** The finds as tiles. Like the list, it redraws only when the finds, picks or selection change. */
export const FindGrid = memo(function FindGrid({ finds, picked, kept, selected, onSelectedChange }: Props) {
  const toggle = usePick(finds, selected, onSelectedChange);
  const chosen = new Set(selected);
  return (
    <ul className="find-grid" aria-label="Found on the page">
      {finds.map((find) => (
        <FindTile
          key={find.url}
          find={find}
          pick={picked[find.url]}
          kept={kept[find.url]}
          chosen={chosen.has(find.url)}
          onToggle={toggle}
        />
      ))}
    </ul>
  );
});

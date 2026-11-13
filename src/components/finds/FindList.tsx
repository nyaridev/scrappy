// FindList: a page of finds as rows (`FindRow`); a click on a row picks it, a shift-click picks a run of rows.

import { memo } from "react";
import type { Find } from "../../lib/types/Find";
import type { Kept } from "../../lib/types/Kept";
import type { Picked } from "../../lib/types/Picked";
import { FindRow } from "./FindRow";
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
/** The finds as rows. It redraws only when the finds, picks or selection change, not as downloads move. */
export const FindList = memo(function FindList({ finds, picked, kept, selected, onSelectedChange }: Props) {
  const toggle = usePick(finds, selected, onSelectedChange);
  const chosen = new Set(selected);
  return (
    <ul className="find-list" aria-label="Found on the page">
      {finds.map((find) => (
        <FindRow
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

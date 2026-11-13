// FindTile: one find in the grid, the whole tile a button that picks it: its thumbnail (its type at the corner, and
// how its download is going laid over it), name and size. Its size never changes.

import { Badge, Icon, Tooltip } from "iro-ui";
import { CheckCircleIcon } from "iro-ui/icons";
import { formatBytes } from "../../lib/format";
import type { Find } from "../../lib/types/Find";
import type { Kept } from "../../lib/types/Kept";
import type { Picked } from "../../lib/types/Picked";
import { FindStatus } from "./FindStatus";
import { FindThumb } from "./FindThumb";
import { FIND_KINDS } from "./findKinds";

// ── Public API ────────────────────────────
/** One find as a tile; a click picks it or drops it, a shift-click picks the run since the last click. */
export function FindTile({
  find,
  pick,
  kept,
  chosen,
  onToggle,
}: {
  find: Find;
  pick?: Picked;
  kept?: Kept;
  chosen: boolean;
  onToggle: (url: string, range: boolean) => void;
}) {
  const look = FIND_KINDS[find.kind];
  return (
    <li className="find-tile" data-selected={chosen || undefined}>
      <button
        type="button"
        className="find-tile-pick"
        aria-pressed={chosen}
        aria-label={`Select ${find.name}`}
        onClick={(event) => onToggle(find.url, event.shiftKey)}
      >
        <span className="find-box find-tile-box">
          <FindThumb find={find} iconSize={28} />
          <Badge className="find-tile-kind" tone={look.tone} icon={<Icon icon={look.icon} />}>
            {look.label}
          </Badge>
          {chosen && <Icon className="find-check" icon={CheckCircleIcon} size={20} />}
          <FindStatus find={find} pick={pick} kept={kept} />
        </span>
        <span className="find-tile-text">
          <Tooltip content={find.url}>
            <span className="find-name find-tile-name">{find.name}</span>
          </Tooltip>
          <span className="find-tile-facts">{find.size != null ? formatBytes(find.size) : "Size unknown"}</span>
        </span>
      </button>
    </li>
  );
}

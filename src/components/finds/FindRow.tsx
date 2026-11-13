// FindRow: one find in the list, the whole row a button that picks it: a pick mark, its thumbnail (how its download
// is going laid over it), type, name and size.

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
/** One find as a row; a click picks it or drops it, a shift-click picks the run since the last click. */
export function FindRow({
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
    <li className="find-row" data-selected={chosen || undefined}>
      <button
        type="button"
        className="find-row-pick"
        aria-pressed={chosen}
        aria-label={`Select ${find.name}`}
        onClick={(event) => onToggle(find.url, event.shiftKey)}
      >
        <span className="find-row-mark">{chosen && <Icon icon={CheckCircleIcon} size={18} />}</span>
        <span className="find-box find-row-box">
          <FindThumb find={find} iconSize={16} />
          <FindStatus find={find} pick={pick} kept={kept} />
        </span>
        <Badge className="find-row-kind" tone={look.tone} icon={<Icon icon={look.icon} />}>
          {look.label}
        </Badge>
        <Tooltip content={find.url}>
          <span className="find-name">{find.name}</span>
        </Tooltip>
        <span className="find-row-size">{find.size != null ? formatBytes(find.size) : ""}</span>
      </button>
    </li>
  );
}

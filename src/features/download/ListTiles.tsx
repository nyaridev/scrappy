// ListTiles: the lists a link holds, one card each: a channel's Videos and Shorts (each with an on/off switch), or a
// playlist's one list. A card shows its first videos' thumbnails and how many are picked; clicking it opens the
// VideoPicker on that list.

import { Icon, Switch } from "iro-ui";
import { PlayIcon } from "iro-ui/icons";
import { type Dispatch, type SetStateAction, useState } from "react";
import { formatCount } from "../../lib/format";
import type { ListEntry } from "../../lib/types/ListEntry";
import type { Section } from "../../lib/types/Section";
import { usePreview } from "./usePreview";
import { VideoPicker } from "./VideoPicker";

// ── Constants ─────────────────────────────
/** How many thumbnails a card shows: two rows of two wide ones, or one row of tall ones. */
const MOSAIC = 4;

// ── Private helpers ───────────────────────
/** `6 videos`, or `4 of 6 videos` when some are turned off. */
function pickedText(list: Section, skip: Set<string>): string {
  const off = list.entries.filter((entry) => skip.has(entry.url)).length;
  const all = formatCount(list.count, "video", list.more);
  return off === 0 ? all : `${list.count - off} of ${all}`;
}

function MosaicPicture({ entry }: { entry: ListEntry }) {
  const [ref, src] = usePreview(entry.url);
  return (
    <span className="mosaic-cell" ref={ref}>
      {src ? <img src={src} alt="" draggable={false} /> : <Icon icon={PlayIcon} size={16} />}
    </span>
  );
}

// ── Public API ────────────────────────────
/** The cards; `on` is null when the lists can't be turned off one by one (a playlist). */
export function ListTiles({
  lists,
  on,
  onOnChange,
  skip,
  onSkipChange,
}: {
  lists: Section[];
  on: string[] | null;
  onOnChange: (on: string[]) => void;
  skip: string[];
  onSkipChange: Dispatch<SetStateAction<string[]>>;
}) {
  const [open, setOpen] = useState<string | null>(null);
  const skipped = new Set(skip);
  return (
    <>
      <ul className="list-tiles" aria-label="Lists">
        {lists.map((list) => {
          const active = !on || on.includes(list.url);
          return (
            <li key={list.url} className="list-tile" data-off={!active || undefined}>
              <button
                type="button"
                className="mosaic"
                data-tall={list.entries[0]?.tall || undefined}
                aria-label={`Choose from ${list.title}`}
                disabled={list.entries.length === 0}
                onClick={() => setOpen(list.url)}
              >
                {list.entries.slice(0, MOSAIC).map((entry) => (
                  <MosaicPicture key={entry.url} entry={entry} />
                ))}
              </button>
              <span className="list-tile-foot">
                {/* The name opens the picker too; the mosaic is the one keyboard stop. */}
                <button
                  type="button"
                  className="list-tile-text"
                  tabIndex={-1}
                  disabled={list.entries.length === 0}
                  onClick={() => setOpen(list.url)}
                >
                  <span className="list-tile-title">{list.title}</span>
                  <span className="list-tile-count">{pickedText(list, skipped)}</span>
                </button>
                {on && (
                  <Switch
                    size="sm"
                    aria-label={`Download ${list.title}`}
                    checked={active}
                    onCheckedChange={(checked) =>
                      onOnChange(checked ? [...on, list.url] : on.filter((url) => url !== list.url))
                    }
                  />
                )}
              </span>
            </li>
          );
        })}
      </ul>
      {open && (
        <VideoPicker lists={lists} first={open} skip={skip} onSkipChange={onSkipChange} onClose={() => setOpen(null)} />
      )}
    </>
  );
}

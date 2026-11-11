// VideoCard: one video in the picker's grid: its thumbnail with its length, a check mark when it's picked, and its
// title and views. Clicking it picks or drops it.

import { Icon } from "iro-ui";
import { CheckIcon, PlayIcon } from "iro-ui/icons";
import { formatDuration, formatViews } from "../../lib/format";
import type { ListEntry } from "../../lib/types/ListEntry";
import { usePreview } from "./usePreview";

/** The card. */
export function VideoCard({ entry, picked, onToggle }: { entry: ListEntry; picked: boolean; onToggle: () => void }) {
  const [ref, src] = usePreview(entry.url);
  return (
    <li className="video-card" data-tall={entry.tall || undefined}>
      <button type="button" className="video-card-button" aria-pressed={picked} onClick={onToggle}>
        <span className="video-card-thumb" ref={ref}>
          {src ? (
            <img src={src} alt="" draggable={false} />
          ) : (
            <span className="video-card-empty" data-loading={src === undefined || undefined}>
              <Icon icon={PlayIcon} size={20} />
            </span>
          )}
          <span className="video-card-check" aria-hidden>
            <Icon icon={CheckIcon} size={14} />
          </span>
          {entry.duration != null && <span className="video-card-length">{formatDuration(entry.duration)}</span>}
        </span>
        <span className="video-card-text">
          <span className="video-card-title">{entry.title}</span>
          {entry.views != null && <span className="video-card-views">{formatViews(entry.views)}</span>}
        </span>
      </button>
    </li>
  );
}

// GalleryTile: one file in the gallery: its thumbnail (or its kind's icon), its type, name and facts, and a
// show-in-folder button. A file gone from disk says so and doesn't open.

import { Badge, Icon, Image, Skeleton, Tooltip } from "iro-ui";
import { useThumbnail } from "../../app/useThumbnail";
import { ShowInFolderButton } from "../../components/ShowInFolderButton";
import { formatBytes, formatDuration } from "../../lib/format";
import { kindOf } from "../../lib/kinds";
import type { GalleryItem } from "../../lib/types/GalleryItem";

/** One tile. */
export function GalleryTile({ item, onOpen }: { item: GalleryItem; onOpen: (item: GalleryItem) => void }) {
  const thumb = useThumbnail(item.missing ? null : item.path);
  const look = kindOf(item.kind);
  const facts = item.missing
    ? ["Missing from the folder"]
    : [item.duration != null ? formatDuration(item.duration) : null, formatBytes(item.bytes), item.site];
  return (
    <li className="tile" data-missing={item.missing || undefined}>
      <button
        type="button"
        className="tile-open"
        disabled={item.missing}
        aria-label={`Open ${item.name}`}
        onClick={() => onOpen(item)}
      >
        {thumb === undefined ? (
          <Skeleton className="tile-thumb" />
        ) : thumb ? (
          <Image className="tile-thumb" src={thumb} alt="" ratio={16 / 9} fit="cover" />
        ) : (
          <span className="tile-thumb tile-thumb-empty">
            <Icon icon={look.icon} size={28} />
          </span>
        )}
        <Badge className="tile-kind" icon={<Icon icon={look.icon} />}>
          {look.one}
        </Badge>
      </button>
      <span className="tile-text">
        <Tooltip content={item.path}>
          <span className="tile-name">{item.name}</span>
        </Tooltip>
        <span className="tile-facts">{facts.filter(Boolean).join(" · ")}</span>
      </span>
      {!item.missing && <ShowInFolderButton path={item.path} />}
    </li>
  );
}

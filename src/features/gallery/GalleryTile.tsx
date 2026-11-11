// GalleryTile: one file in the gallery: its thumbnail (or its kind's icon), its type, name and facts, and a
// show-in-folder button. Right-click: Open, Show in folder.

import { Badge, ContextMenu, Icon, Image, Skeleton, Tooltip } from "iro-ui";
import { memo } from "react";
import { useThumbnail } from "../../app/useThumbnail";
import { fileMenu } from "../../components/fileMenu";
import { ShowInFolderButton } from "../../components/ShowInFolderButton";
import { formatBytes, formatDuration } from "../../lib/format";
import { kindOf } from "../../lib/kinds";
import type { GalleryItem } from "../../lib/types/GalleryItem";

/** One tile, drawn again only when its file changes: `onOpen` only uses state setters, so a new one does the same. */
export const GalleryTile = memo(
  function GalleryTile({ item, onOpen }: { item: GalleryItem; onOpen: (item: GalleryItem) => void }) {
    const thumb = useThumbnail(item.path);
    const look = kindOf(item.kind);
    const facts = [item.duration != null ? formatDuration(item.duration) : null, formatBytes(item.bytes), item.site];
    return (
      <ContextMenu items={fileMenu(item.path, () => onOpen(item))}>
        <li className="tile">
          <button type="button" className="tile-open" aria-label={`Open ${item.name}`} onClick={() => onOpen(item)}>
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
          <ShowInFolderButton path={item.path} />
        </li>
      </ContextMenu>
    );
  },
  (before, after) =>
    before.item.path === after.item.path &&
    before.item.bytes === after.item.bytes &&
    before.item.kind === after.item.kind &&
    before.item.site === after.item.site &&
    before.item.duration === after.item.duration &&
    before.item.downloadedAt === after.item.downloadedAt,
);

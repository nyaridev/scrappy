// GalleryTile: one file in the gallery: its thumbnail (or its kind's icon), its type, name and facts, and a
// show-in-folder button. Right-click: Open, Show in folder, Delete.

import { Badge, ContextMenu, Icon, Image, Skeleton, Tooltip } from "iro-ui";
import { TrashIcon } from "iro-ui/icons";
import { type CSSProperties, memo } from "react";
import { useThumbnail } from "../../app/useThumbnail";
import { fileMenu } from "../../components/fileMenu";
import { ShowInFolderButton } from "../../components/ShowInFolderButton";
import { formatBytes, formatDuration } from "../../lib/format";
import { kindOf } from "../../lib/kinds";
import type { GalleryItem } from "../../lib/types/GalleryItem";

/** One tile, drawn again only when its file changes: `onOpen` and `onDelete` only use state setters, so new ones do
 * the same. */
export const GalleryTile = memo(
  function GalleryTile({
    item,
    index,
    onOpen,
    onDelete,
  }: {
    item: GalleryItem;
    /** Its place in the grid: the first rows arrive one after another. */
    index: number;
    onOpen: (item: GalleryItem) => void;
    /** Asks to delete it; the view confirms first. */
    onDelete: (item: GalleryItem) => void;
  }) {
    const thumb = useThumbnail(item.path);
    const look = kindOf(item.kind);
    const facts = [item.duration != null ? formatDuration(item.duration) : null, formatBytes(item.bytes), item.site];
    return (
      <ContextMenu
        items={[
          ...fileMenu(item.path, () => onOpen(item)),
          "separator",
          { label: "Delete", icon: <Icon icon={TrashIcon} />, danger: true, onSelect: () => onDelete(item) },
        ]}
      >
        <li className="tile" data-iro-animate="fade-up" style={{ "--iro-index": index } as CSSProperties}>
          <button type="button" className="tile-open" aria-label={`Open ${item.name}`} onClick={() => onOpen(item)}>
            {thumb === undefined ? (
              <Skeleton className="tile-thumb" height="auto" />
            ) : thumb ? (
              <Image className="tile-thumb" src={thumb} alt="" ratio={16 / 9} fit="contain" />
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
    before.index === after.index &&
    before.item.path === after.item.path &&
    before.item.bytes === after.item.bytes &&
    before.item.kind === after.item.kind &&
    before.item.site === after.item.site &&
    before.item.duration === after.item.duration &&
    before.item.downloadedAt === after.item.downloadedAt,
);

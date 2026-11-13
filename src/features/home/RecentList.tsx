// RecentList: the latest downloads, newest first, as tiles like the Gallery's: the thumbnail opens the file in its
// app, the button shows it in the file manager. Right-click: Open, Show in folder.

import { Badge, ContextMenu, Icon, Image, Skeleton, Tooltip } from "iro-ui";
import type { CSSProperties } from "react";
import { useThumbnail } from "../../app/useThumbnail";
import { fileMenu, openInApp } from "../../components/fileMenu";
import { ShowInFolderButton } from "../../components/ShowInFolderButton";
import { formatBytes, formatWhen } from "../../lib/format";
import { kindOf } from "../../lib/kinds";
import type { RecentDownload } from "../../lib/types/RecentDownload";

// ── Private helpers ───────────────────────
/** One recent download. */
function RecentTile({ download, index }: { download: RecentDownload; index: number }) {
  const thumb = useThumbnail(download.path);
  const look = kindOf(download.kind);
  const facts = [formatWhen(download.downloadedAt), formatBytes(download.bytes), download.site];
  return (
    <ContextMenu items={fileMenu(download.path)}>
      <li className="tile" data-iro-animate="fade-up" style={{ "--iro-index": index } as CSSProperties}>
        <button
          type="button"
          className="tile-open"
          aria-label={`Open ${download.name}`}
          onClick={() => openInApp(download.path)}
        >
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
          <Tooltip content={download.path}>
            <span className="tile-name">{download.name}</span>
          </Tooltip>
          <span className="tile-facts">{facts.filter(Boolean).join(" · ")}</span>
        </span>
        <ShowInFolderButton path={download.path} />
      </li>
    </ContextMenu>
  );
}

// ── Public API ────────────────────────────
/** The recent downloads. */
export function RecentList({ downloads }: { downloads: RecentDownload[] }) {
  return (
    <ul className="home-tiles" aria-label="Recent downloads">
      {downloads.map((download, index) => (
        <RecentTile key={download.path} download={download} index={index} />
      ))}
    </ul>
  );
}

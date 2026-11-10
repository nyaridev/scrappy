// DuplicateGroup: one group of copies side by side, the best first and marked; a checkbox on each picks it for the
// trash.

import { Badge, Checkbox, Icon, Image, Skeleton, Tooltip } from "iro-ui";
import { ImageIcon } from "iro-ui/icons";
import { useThumbnail } from "../app/useThumbnail";
import { formatBytes } from "../lib/format";
import type { Picture } from "../lib/types/Picture";

// ── Private helpers ───────────────────────
function Copy({
  picture,
  best,
  picked,
  onPick,
}: {
  picture: Picture;
  best: boolean;
  picked: boolean;
  onPick: (on: boolean) => void;
}) {
  const thumb = useThumbnail(picture.path);
  const facts = [
    `${picture.width}×${picture.height}`,
    picture.quality != null ? `${picture.format} ${picture.quality}%` : picture.format,
    formatBytes(picture.bytes),
  ];
  return (
    <li className="copy" data-picked={picked || undefined}>
      <span className="copy-thumb">
        {thumb === undefined ? (
          <Skeleton className="tile-thumb" />
        ) : thumb ? (
          <Image className="tile-thumb" src={thumb} alt="" ratio={16 / 9} fit="contain" />
        ) : (
          <span className="tile-thumb tile-thumb-empty">
            <Icon icon={ImageIcon} size={24} />
          </span>
        )}
        {best && (
          <Badge className="tile-kind" tone="success">
            Best
          </Badge>
        )}
      </span>
      <Checkbox
        checked={picked}
        onCheckedChange={onPick}
        label={
          <Tooltip content={picture.path}>
            <span className="tile-name">{picture.name}</span>
          </Tooltip>
        }
        description={facts.join(" · ")}
      />
    </li>
  );
}

// ── Public API ────────────────────────────
/** The group. */
export function DuplicateGroup({
  group,
  picked,
  onPick,
}: {
  group: Picture[];
  picked: Set<string>;
  onPick: (path: string, on: boolean) => void;
}) {
  return (
    <ul className="copies" aria-label={`Copies of ${group[0]?.name ?? "a picture"}`}>
      {group.map((picture, index) => (
        <Copy
          key={picture.path}
          picture={picture}
          best={index === 0}
          picked={picked.has(picture.path)}
          onPick={(on) => onPick(picture.path, on)}
        />
      ))}
    </ul>
  );
}

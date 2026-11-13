// FindThumb: a find's thumbnail from Rust (a picture scaled down, a video's or stream's frame), or its type's icon for
// audio, files and anything that can't be read. It fills its box, so the box keeps its size while it loads.

import { Icon, Image, Skeleton } from "iro-ui";
import { useCallback } from "react";
import { useThumbnail } from "../../app/useThumbnail";
import { findThumbnail } from "../../lib/backend";
import type { Find } from "../../lib/types/Find";
import { FIND_KINDS } from "./findKinds";

// ── Public API ────────────────────────────
/** A find's picture, filling the box around it. */
export function FindThumb({ find, iconSize }: { find: Find; iconSize: number }) {
  const pictured = find.kind !== "audio" && find.kind !== "other";
  const load = useCallback((url: string) => findThumbnail(url, find.kind), [find.kind]);
  const thumb = useThumbnail(pictured ? find.url : null, load);
  if (thumb === undefined) return <Skeleton className="find-thumb" width="100%" height="100%" />;
  // Shown whole, its own shape kept: a tall or wide one gets dark edges instead of being cut.
  if (thumb) return <Image className="find-thumb" src={thumb} alt="" ratio={16 / 9} fit="contain" />;
  return (
    <span className="find-thumb find-thumb-empty">
      <Icon icon={FIND_KINDS[find.kind].icon} size={iconSize} />
    </span>
  );
}

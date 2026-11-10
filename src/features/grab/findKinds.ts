// Find kinds: each kind's label, icon and badge color. The label always shows, so color is never the only signal.

import type { IconData } from "iro-ui/icons";
import { AudioBarsIcon, BroadcastIcon, FileIcon, ImageIcon, PlayIcon } from "iro-ui/icons";
import type { FindKind } from "../../lib/types/FindKind";

// ── Types ─────────────────────────────────
export interface KindLook {
  label: string;
  icon: IconData;
  tone: "neutral" | "accent" | "success" | "warning" | "info";
}

// ── Public API ────────────────────────────
/** Every kind, in the order the filters list them. */
export const FIND_KINDS: Record<FindKind, KindLook> = {
  video: { label: "Video", icon: PlayIcon, tone: "accent" },
  stream: { label: "Stream", icon: BroadcastIcon, tone: "warning" },
  audio: { label: "Audio", icon: AudioBarsIcon, tone: "info" },
  image: { label: "Image", icon: ImageIcon, tone: "success" },
  other: { label: "File", icon: FileIcon, tone: "neutral" },
};

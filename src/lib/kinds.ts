// Kinds: the labels and icon for each download kind the catalog keeps (ADR-0004), for Home and the Gallery.

import type { IconData } from "iro-ui/icons";
import { AudioBarsIcon, FileIcon, GlobeIcon, ImageIcon, PlayIcon } from "iro-ui/icons";

// ── Types ─────────────────────────────────
export interface KindLook {
  /** For a group: "Videos". */
  label: string;
  /** For one file: "Video". */
  one: string;
  icon: IconData;
}

// ── Constants ─────────────────────────────
const KINDS: Record<string, KindLook> = {
  video: { label: "Videos", one: "Video", icon: PlayIcon },
  audio: { label: "Audio", one: "Audio", icon: AudioBarsIcon },
  social: { label: "Social", one: "Social", icon: PlayIcon },
  image: { label: "Images", one: "Image", icon: ImageIcon },
  file: { label: "Files", one: "File", icon: FileIcon },
  site: { label: "Sites", one: "Site", icon: GlobeIcon },
};

// ── Public API ────────────────────────────
/** The kind's label and icon; an unknown kind shows its own name. */
export function kindOf(kind: string): KindLook {
  return KINDS[kind] ?? { label: kind, one: kind, icon: FileIcon };
}

// Find kinds: each kind's label, icon, badge color and what it is (a filter's tooltip). The label always shows, so
// color is never the only signal.

import type { IconData } from "iro-ui/icons";
import { AudioBarsIcon, BroadcastIcon, FileIcon, ImageIcon, PlayIcon } from "iro-ui/icons";
import type { FindKind } from "../../lib/types/FindKind";

// ── Types ─────────────────────────────────
export interface KindLook {
  label: string;
  icon: IconData;
  tone: "neutral" | "accent" | "success" | "warning" | "info";
  /** What the kind holds, on hover. */
  about: string;
}

// ── Public API ────────────────────────────
/** Every kind, in the order the filters list them. */
export const FIND_KINDS: Record<FindKind, KindLook> = {
  video: { label: "Video", icon: PlayIcon, tone: "accent", about: "Video files: MP4, WebM, MOV and the like." },
  stream: {
    label: "Stream",
    icon: BroadcastIcon,
    tone: "warning",
    about: "Video sent in pieces (HLS or DASH), saved as one MP4 through yt-dlp.",
  },
  audio: { label: "Audio", icon: AudioBarsIcon, tone: "info", about: "Sound files: MP3, M4A, OGG, WAV and the like." },
  image: {
    label: "Image",
    icon: ImageIcon,
    tone: "success",
    about: "Pictures: JPEG, PNG, WebP, GIF, AVIF, SVG and the like.",
  },
  other: {
    label: "File",
    icon: FileIcon,
    tone: "neutral",
    about: "Other downloads: PDFs, archives (ZIP, RAR, 7z), documents, installers and 3D models.",
  },
};

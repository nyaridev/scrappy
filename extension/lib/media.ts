// Media: what the extension found on a page (from the page itself or from its network requests), how each link is
// labelled, and merging the two lists. Shared by the background script and the popup.

import type { Header } from "./types/Header";
import type { MediaKind } from "./types/MediaKind";

// ── Constants ─────────────────────────────
const FORMATS: Record<string, string> = {
  mp4: "MP4",
  m4v: "MP4",
  webm: "WEBM",
  mov: "MOV",
  mkv: "MKV",
  ogv: "OGG",
  m3u8: "HLS",
  mpd: "DASH",
  mp3: "MP3",
  m4a: "M4A",
  ogg: "OGG",
  opus: "OPUS",
  wav: "WAV",
  flac: "FLAC",
  jpg: "JPG",
  jpeg: "JPG",
  png: "PNG",
  webp: "WEBP",
  gif: "GIF",
  avif: "AVIF",
  svg: "SVG",
  bmp: "BMP",
};

const MIME_FORMATS: Record<string, string> = {
  "application/vnd.apple.mpegurl": "HLS",
  "application/x-mpegurl": "HLS",
  "audio/mpegurl": "HLS",
  "audio/x-mpegurl": "HLS",
  "application/dash+xml": "DASH",
  "video/mp4": "MP4",
  "video/webm": "WEBM",
  "video/quicktime": "MOV",
  "video/x-matroska": "MKV",
  "video/ogg": "OGG",
  "audio/mpeg": "MP3",
  "audio/mp4": "M4A",
  "audio/ogg": "OGG",
  "audio/wav": "WAV",
  "audio/flac": "FLAC",
  "image/jpeg": "JPG",
  "image/png": "PNG",
  "image/webp": "WEBP",
  "image/gif": "GIF",
  "image/avif": "AVIF",
  "image/svg+xml": "SVG",
};

/** Pieces of a stream, never worth sending on their own. */
const SEGMENTS = /\.(ts|m4s|aac|cmfv|cmfa)$/;
const SEGMENT_TYPES = ["video/mp2t", "video/iso.segment", "audio/iso.segment"];

// ── Types ─────────────────────────────────
/** One video or image the extension found. */
export interface Found {
  url: string;
  kind: MediaKind;
  /** `MP4`, `HLS`, `JPG`…, or null when neither the link nor the type says. */
  format: string | null;
  width?: number;
  height?: number;
  /** Bytes, from the response. */
  size?: number;
  /** The response's type, when the request was seen. */
  mime?: string;
  /** A picture to show for it: the image itself, or a video's poster. */
  preview?: string;
  /** The request's allowed headers (Referer, Origin, User-Agent); the app checks them again. */
  headers?: Header[];
}

// ── Private helpers ───────────────────────
function extensionOf(url: string): string {
  try {
    const path = new URL(url).pathname.toLowerCase();
    return path.slice(path.lastIndexOf(".") + 1);
  } catch {
    return "";
  }
}

// ── Public API ────────────────────────────
/** The format label from the response type, else the link's extension. */
export function formatOf(url: string, mime?: string): string | null {
  const type = (mime?.split(";")[0] ?? "").trim().toLowerCase();
  return MIME_FORMATS[type] ?? FORMATS[extensionOf(url)] ?? null;
}

/** Whether a network response is a video or audio worth listing: a file or a playlist, not a stream's pieces or
 * YouTube's chunks (the page link covers those). */
export function isMediaResponse(url: string, mime: string): boolean {
  const type = (mime.split(";")[0] ?? "").trim().toLowerCase();
  if (SEGMENT_TYPES.includes(type) || SEGMENTS.test(new URL(url).pathname.toLowerCase())) return false;
  if (url.includes("googlevideo.com/videoplayback")) return false;
  if (
    type.startsWith("video/") ||
    type.startsWith("audio/") ||
    MIME_FORMATS[type] === "HLS" ||
    type === "application/dash+xml"
  )
    return true;
  return ["m3u8", "mpd", "mp4", "webm", "mov", "m4a", "mp3"].includes(extensionOf(url));
}

/** The full size from `Content-Range` (`bytes 0-1/123`), else `Content-Length`. */
export function sizeOf(contentRange?: string, contentLength?: string): number | undefined {
  const total = Number(contentRange?.split("/")[1]);
  if (total > 0) return total;
  const length = Number(contentLength);
  return length > 0 ? length : undefined;
}

/** One list with each link once; a later find fills in what an earlier one lacked. */
export function merge(...lists: Found[][]): Found[] {
  const byUrl = new Map<string, Found>();
  for (const found of lists.flat()) {
    const known = byUrl.get(found.url);
    byUrl.set(
      found.url,
      known ? { ...found, ...known, ...Object.fromEntries(Object.entries(found).filter(([, v]) => v != null)) } : found,
    );
  }
  return [...byUrl.values()];
}

/** The last part of the link's path, for a row's name: `.../Sunset%20sky.jpg?w=2` → `Sunset sky.jpg`. */
export function nameOf(url: string): string {
  try {
    const parts = new URL(url).pathname.split("/").filter(Boolean);
    return decodeURIComponent(parts.at(-1) ?? new URL(url).host);
  } catch {
    return url;
  }
}

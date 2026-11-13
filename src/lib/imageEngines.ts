// Image engines: the image search engines Hunt can search, with their names (ADR-0025), for Hunt and Settings.

import type { ImageEngine } from "./types/ImageEngine";

// ── Public API ────────────────────────────
/** Every engine, in the order they're listed. */
export const IMAGE_ENGINES: { engine: ImageEngine; label: string }[] = [
  { engine: "duckDuckGo", label: "DuckDuckGo Images" },
  { engine: "bing", label: "Bing Images" },
  { engine: "yandex", label: "Yandex Images" },
];

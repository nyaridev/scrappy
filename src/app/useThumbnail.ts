// useThumbnail: a file's thumbnail from Rust, asked for once per session and kept.

import { useEffect, useState } from "react";
import { thumbnail } from "../lib/backend";

// ── State ─────────────────────────────────
/** By path: the `data:` URL, or `null` for a file with no picture. */
const thumbnails = new Map<string, string | null>();

// ── Public API ────────────────────────────
/** The thumbnail of the file at `path`: `undefined` while it loads, `null` when there's none. */
export function useThumbnail(path: string | null): string | null | undefined {
  const [url, setUrl] = useState(() => (path ? thumbnails.get(path) : null));
  useEffect(() => {
    if (!path) return setUrl(null);
    if (thumbnails.has(path)) return setUrl(thumbnails.get(path));
    let current = true;
    setUrl(undefined);
    thumbnail(path)
      .catch(() => null)
      .then((fresh) => {
        thumbnails.set(path, fresh);
        if (current) setUrl(fresh);
      });
    return () => {
      current = false;
    };
  }, [path]);
  return url;
}

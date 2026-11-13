// useThumbnail: a library file's thumbnail (or a link's, with its own loader) from Rust, asked for once per session
// and kept.

import { useEffect, useState } from "react";
import { thumbnail } from "../lib/backend";

// ── State ─────────────────────────────────
/** By path or link: the `data:` URL, or `null` when there's no picture. */
const thumbnails = new Map<string, string | null>();

// ── Public API ────────────────────────────
/** The thumbnail of the file at `path` (or the link `load` reads): `undefined` while it loads, `null` when there's
 * none. `load` must keep its identity between renders. */
export function useThumbnail(
  path: string | null,
  load: (path: string) => Promise<string | null> = thumbnail,
): string | null | undefined {
  const [url, setUrl] = useState(() => (path ? thumbnails.get(path) : null));
  useEffect(() => {
    if (!path) return setUrl(null);
    if (thumbnails.has(path)) return setUrl(thumbnails.get(path));
    let current = true;
    setUrl(undefined);
    load(path)
      .catch(() => null)
      .then((fresh) => {
        thumbnails.set(path, fresh);
        if (current) setUrl(fresh);
      });
    return () => {
      current = false;
    };
  }, [path, load]);
  return url;
}

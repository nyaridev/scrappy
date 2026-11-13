// useMirrorMedia: a mirror's images and videos to pick from (ADR-0033), read again every second while its crawl or a
// save runs, and saving the picked ones into the mirror.

import { useCallback, useEffect, useState } from "react";
import { mirrorMedia, saveMirrorMedia } from "../../lib/backend";
import type { MirrorMedia } from "../../lib/types/MirrorMedia";

// ── Constants ─────────────────────────────
const READ_EVERY_MS = 1000;

// ── Public API ────────────────────────────
/** The media of `site`'s mirror (`null` before there's any), and saving picks; `crawling` while its crawl runs. */
export function useMirrorMedia(site: string | null, crawling: boolean) {
  const [media, setMedia] = useState<MirrorMedia | null>(null);
  const busy = crawling || (media?.busy ?? false);

  useEffect(() => {
    if (!site) {
      setMedia(null);
      return;
    }
    let live = true;
    // A crawl that just started may not have its mirror yet: the next read finds it.
    const read = () =>
      mirrorMedia(site)
        .then((next) => live && setMedia(next))
        .catch(() => {});
    void read();
    const timer = busy ? setInterval(read, READ_EVERY_MS) : undefined;
    return () => {
      live = false;
      clearInterval(timer);
    };
  }, [site, busy]);

  const save = useCallback(
    async (urls: string[], maxVideoMb: number) => {
      if (!site) return;
      await saveMirrorMedia({ site, urls, maxVideoMb });
      setMedia(await mirrorMedia(site));
    },
    [site],
  );

  return { media, busy, save };
}

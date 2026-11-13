// Scan: runs inside the page (through `scripting.executeScript`) and lists its videos and images: video and audio
// elements, images (the largest `srcset` candidate), social preview tags and CSS backgrounds, with the sizes the page's
// resource timing knows. It must stay self-contained: the browser copies only this function's source into the page.

import type { Found } from "./media";

/** The page's media, without tiny images (icons, tracking pixels). */
export function scanPage(): Found[] {
  const MIN_SIDE = 48;
  const MAX_ELEMENTS = 4000;
  const found: Found[] = [];
  const seen = new Set<string>();

  const absolute = (link: string | null | undefined): string | null => {
    if (!link) return null;
    try {
      const url = new URL(link, location.href);
      return url.protocol === "http:" || url.protocol === "https:" ? url.href : null;
    } catch {
      return null;
    }
  };
  // Cross-origin files without `Timing-Allow-Origin` report 0: no size then.
  const bytesOf = (url: string): number | undefined => {
    const entry = performance.getEntriesByName(url).at(-1) as PerformanceResourceTiming | undefined;
    return entry?.encodedBodySize || entry?.decodedBodySize || undefined;
  };
  const add = (link: string | null | undefined, item: Omit<Found, "url" | "format">) => {
    const url = absolute(link);
    if (!url || seen.has(url)) return;
    seen.add(url);
    found.push({ url, format: null, size: bytesOf(url), ...item });
  };
  const largestInSrcset = (srcset: string): string | null => {
    let best: { url: string; width: number } | null = null;
    for (const candidate of srcset.split(",")) {
      const [url, descriptor = "1x"] = candidate.trim().split(/\s+/);
      const width = Number.parseFloat(descriptor) * (descriptor.endsWith("x") ? 1000 : 1);
      if (url && (!best || width > best.width)) best = { url, width };
    }
    return best?.url ?? null;
  };

  for (const media of document.querySelectorAll<HTMLMediaElement>("video, audio")) {
    const video = media instanceof HTMLVideoElement ? media : null;
    const item = {
      kind: "video" as const,
      width: video?.videoWidth || undefined,
      height: video?.videoHeight || undefined,
      preview: absolute(video?.poster) ?? undefined,
      duration: Number.isFinite(media.duration) && media.duration > 0 ? media.duration : undefined,
    };
    add(media.currentSrc || media.src, item);
    for (const source of media.querySelectorAll("source")) add(source.src, item);
  }

  for (const image of document.images) {
    if (image.naturalWidth && image.naturalWidth < MIN_SIDE && image.naturalHeight < MIN_SIDE) continue;
    const best = (image.srcset && largestInSrcset(image.srcset)) || image.currentSrc || image.src;
    const url = absolute(best);
    add(best, {
      kind: "image",
      width: image.naturalWidth || undefined,
      height: image.naturalHeight || undefined,
      preview: url ?? undefined,
    });
  }

  const meta = (selector: string) => document.querySelector<HTMLMetaElement>(selector)?.content;
  add(meta('meta[property="og:video"], meta[property="og:video:url"], meta[property="og:video:secure_url"]'), {
    kind: "video",
  });
  for (const selector of ['meta[property="og:image"]', 'meta[name="twitter:image"]']) {
    const url = absolute(meta(selector));
    add(url, { kind: "image", preview: url ?? undefined });
  }

  const elements = document.body?.getElementsByTagName("*") ?? [];
  for (let index = 0; index < Math.min(elements.length, MAX_ELEMENTS); index++) {
    const element = elements[index] as HTMLElement;
    if (element.offsetWidth < MIN_SIDE || element.offsetHeight < MIN_SIDE) continue;
    const background = getComputedStyle(element).backgroundImage;
    for (const match of background.matchAll(/url\(["']?([^"')]+)["']?\)/g)) {
      const url = absolute(match[1]);
      add(url, { kind: "image", preview: url ?? undefined });
    }
  }
  return found;
}

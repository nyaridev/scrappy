// fakeData: the data `backend.ts` answers with outside the app (`pnpm dev` in a browser), the helpers that make it,
// and a fake job that reports progress. The state the fake answers change starts from the `…AtStart` values here
// and is kept in `backend.ts`.
// Preview flags:
// - first run: `?setup`, `?notEmpty` (the picked folder isn't empty), `?missing` (the library folder is gone);
// - tools: `?tools` (nothing installed), `?toolFails` (Deno's download fails), `?outdated` (Scrappy's yt-dlp and a
//   system FFmpeg are older than the pins);
// - links: `?probeFails` (a link check fails), `?signIn` (a link check needs a sign-in); links with `playlist` or
//   `list=` (a 40-video playlist; with `v=` too, read from one of its videos), with `list=RD` (a 450-video Mix), with
//   `/@` (a channel: Videos and Shorts), to x.com, instagram.com or bsky.app (a profile with 50+ files); a list grows
//   over four seconds;
// - library: `?empty` (no downloads yet, no signed-in sites; otherwise two kept downloads), `?manyDownloads` (120 more
//   rows in the Download list, to page through);
// - Grab: `?scanFails` (the scan finds no browser), `?handed` (the extension handed a page at start), `?bigPage`
//   (the scan finds 1300 images);
// - Scrape: a mirror lists 8 clips and 120 pictures as it crawls; saving its picks skips every other clip as too big;
// - window and extension: `?overlay` (macOS title bar), `?native` (system title bar), `?noHost` (no scrappy-host
//   beside the app);
// - Hunt: `?packProblem` (a user's pack can't be read);
// - updates: `?update` (Scrappy 0.2.0 is out).
// The default is a ready library, every tool installed and the custom title bar.

import type { DownloadEntry } from "./types/DownloadEntry";
import type { Find } from "./types/Find";
import type { FolderCheck } from "./types/FolderCheck";
import type { GalleryFolder } from "./types/GalleryFolder";
import type { GalleryItem } from "./types/GalleryItem";
import type { JobFinished } from "./types/JobFinished";
import type { JobProgress } from "./types/JobProgress";
import type { LibraryStatus } from "./types/LibraryStatus";
import type { ListEntry } from "./types/ListEntry";
import type { Packs } from "./types/Packs";
import type { PageScan } from "./types/PageScan";
import type { Picture } from "./types/Picture";
import type { PresetInfo } from "./types/PresetInfo";
import type { Probe } from "./types/Probe";
import type { RecentDownload } from "./types/RecentDownload";
import type { Settings } from "./types/Settings";
import type { Stats } from "./types/Stats";
import type { Tool } from "./types/Tool";
import type { ToolStatus } from "./types/ToolStatus";
import type { WindowFrame } from "./types/WindowFrame";

// ── Fake data ─────────────────────────────
export const flags = new URLSearchParams(window.location.search);
export const fakeRoot = "D:/Scrappy";
export const fakeFrame: WindowFrame = flags.has("native") ? "native" : flags.has("overlay") ? "overlay" : "custom";
export const fakeLibraryAtStart: LibraryStatus =
  flags.has("setup") || flags.has("notEmpty")
    ? { state: "notSet" }
    : flags.has("missing")
      ? { state: "missing", root: "E:/Scrappy" }
      : { state: "ready", root: fakeRoot };
export const fakeSettingsAtStart: Settings = {
  libraryRoot: fakeLibraryAtStart.state === "notSet" ? null : fakeLibraryAtStart.root,
  look: { nativeFrame: fakeFrame === "native", liquidBackground: true },
  downloads: { atOnce: 10, speedLimit: 0 },
  player: { volume: 1, muted: false },
  hiddenPacks: [],
  preferences: {
    media: {
      subtitles: false,
      subtitleLanguages: "en",
      autoSubtitles: false,
      embedMetadata: false,
      embedThumbnail: false,
      embedChapters: false,
      preset: null,
      maxHeight: null,
      nameTemplate: "{site}/{channel}/{title} [{id}]",
    },
    proxy: "",
    starts: {
      grab: { minKb: 10, removeDuplicates: true },
      scrape: { depth: 2, pages: 100, delayMs: 500, maxVideoMb: 200 },
      hunt: { pages: 10, perSource: 50, minKb: 20, engines: ["duckDuckGo", "bing", "yandex"] },
    },
    duplicates: "strict",
    updateYtDlp: true,
    startWithSystem: false,
    checkUpdates: true,
    closeToTray: true,
  },
};

/** Rust's answers, roughly: rejects with a plain message, as `invoke` does. */
export async function fakeCheck(input: string): Promise<FolderCheck> {
  const path = input.trim();
  if (!path) return Promise.reject("Enter a folder.");
  if (!/^([a-z]:[\\/]|\/)/i.test(path)) return Promise.reject("Not a full path.");
  return { path, exists: true, writable: true, empty: !flags.has("notEmpty"), library: false };
}

const toolsMissing = ["setup", "notEmpty", "missing", "tools", "toolFails"].some((flag) => flags.has(flag));
const pins: Record<Tool, string> = { "yt-dlp": "2026.10.26", deno: "2.9.8", "gallery-dl": "1.33.0", ffmpeg: "9.0.2" };
export const fakeToolsAtStart: ToolStatus[] = (Object.keys(pins) as Tool[]).map((tool) => ({
  tool,
  source: toolsMissing ? "missing" : "scrappy",
  version: toolsMissing ? null : pins[tool],
  pinnedVersion: pins[tool],
  outdated: false,
  downloadable: true,
  job: null,
}));
if (flags.has("outdated")) {
  fakeToolsAtStart[0] = { ...fakeToolsAtStart[0], version: "2025.01.01", outdated: true };
  fakeToolsAtStart[3] = { ...fakeToolsAtStart[3], source: "system", version: "7.1-full_build", outdated: true };
}
export const fakeProgress = new Set<(progress: JobProgress) => void>();
export const fakeFinished = new Set<(finished: JobFinished) => void>();
/** Fake jobs asked to stop. */
export const fakeStopped = new Set<number>();

export const fakePresets: PresetInfo[] = [
  { id: "editor-mp4", name: "MP4 (H.264)", audio: false },
  { id: "original", name: "MKV (original)", audio: false },
  { id: "hevc-mp4", name: "MP4 (HEVC)", audio: false },
  { id: "prores-mov", name: "MOV (ProRes)", audio: false },
  { id: "mp3", name: "MP3 (320 kbps)", audio: true },
  { id: "m4a", name: "M4A (AAC)", audio: true },
  { id: "flac", name: "FLAC", audio: true },
  { id: "wav", name: "WAV", audio: true },
];
export const fakeThumbnail = `data:image/svg+xml,${encodeURIComponent(
  '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 320 180"><defs><linearGradient id="g"><stop stop-color="#3a5f4a"/><stop offset="1" stop-color="#8fae6e"/></linearGradient></defs><rect width="320" height="180" fill="url(#g)"/></svg>',
)}`;
const fakeKept = {
  source: "download" as const,
  types: [],
  thumbnail: fakeThumbnail,
  files: null,
  skipped: null,
  preset: "MP4 (H.264)",
};
/** A row of another view's job: no thumbnail, saved files counted. */
export const fakeOther = {
  thumbnail: null,
  skipped: null,
  size: null,
  error: null,
  state: "done" as const,
  types: [],
};
export const fakeDownloadsAtStart: DownloadEntry[] = flags.has("empty")
  ? []
  : [
      {
        ...fakeKept,
        id: 901,
        url: "https://www.youtube.com/watch?v=dQw4w9WgXcQ",
        title: "An older download",
        state: "done",
        file: "Videos/someone/An older download [dQw4w9WgXcQ].mp4",
        size: 48_300_000,
        error: null,
      },
      {
        ...fakeKept,
        id: 902,
        url: "https://vimeo.com/1",
        title: "A failed one",
        state: "failed",
        file: "Videos/someone",
        size: null,
        error: "Download failed: connection reset",
      },
      {
        ...fakeKept,
        id: 906,
        url: "https://x.com/someone/status/1",
        title: "A silent clip",
        preset: "MP3",
        state: "done",
        file: "Audio/someone/A silent clip [1].mp4",
        size: 3_100_000,
        error: "It has no audio, so it was kept as downloaded.",
      },
      {
        ...fakeOther,
        id: 903,
        source: "scrape",
        url: "https://example.com/",
        title: "example.com",
        preset: "Mirror",
        file: "Sites/example.com",
        files: 214,
      },
      {
        ...fakeOther,
        id: 904,
        source: "hunt",
        url: "",
        title: "red pandas",
        preset: "Images",
        file: "Images/red pandas",
        files: 40,
      },
      {
        ...fakeOther,
        id: 905,
        source: "grab",
        url: "https://example.com/files/manual.pdf",
        title: "manual.pdf",
        preset: "Original file",
        file: "Grabs/example.com/2026-11-10/manual.pdf",
        files: null,
        types: ["PDF"],
        size: 2_400_000,
      },
      ...Array.from({ length: flags.has("manyDownloads") ? 120 : 0 }, (_, n) => ({
        ...fakeOther,
        id: 1000 + n,
        source: "grab" as const,
        url: `https://example.com/files/photo-${n + 1}.jpg`,
        title: `photo-${n + 1}.jpg`,
        preset: "Original file",
        file: `Grabs/example.com/2026-11-10/photo-${n + 1}.jpg`,
        files: null,
        types: ["JPG"],
        size: 240_000,
      })),
    ];

const fakeEmpty = flags.has("empty");
export const fakeRecent: RecentDownload[] = fakeEmpty
  ? []
  : [
      ["Videos/jawed/Me at the zoo [jNQXAC9IVRw].mp4", "video", "youtube", 791_000, 0.1],
      ["Audio/Lofi Girl/lofi hip hop radio [jfKfPfyJRdk].mp3", "audio", "youtube", 9_400_000, 3],
      ["Videos/NASA/Artemis II launch [nA9UZF-SZoQ].mp4", "video", "youtube", 412_000_000, 26],
      ["Videos/Vimeo Staff/The Mountain [22439234].mp4", "video", "vimeo", 98_000_000, 50],
      ["Audio/Field Recordings/Rain on a tin roof [8812].flac", "audio", "soundcloud", 61_000_000, 170],
    ].map(([path, kind, site, bytes, hoursAgo]) => ({
      path: path as string,
      name: (path as string).replace(/^.*\/|\.[^.]+$/g, ""),
      kind: kind as string,
      site: site as string,
      bytes: bytes as number,
      downloadedAt: Math.floor(Date.now() / 1000 - (hoursAgo as number) * 3600),
    }));
const fakeDays = Array.from({ length: 30 }, (_, index) => {
  const day = new Date(Date.now() - (29 - index) * 86_400_000);
  const files = fakeEmpty ? 0 : [0, 2, 5, 1, 0, 3, 8][index % 7];
  return { day: day.toISOString().slice(0, 10), files, bytes: files * 70_000_000 };
});
export const fakeStats: Stats = {
  files: fakeEmpty ? 0 : 80,
  bytes: fakeEmpty ? 0 : 5_525_000_000,
  kinds: fakeEmpty
    ? []
    : [
        { name: "video", files: 61, bytes: 5_100_000_000 },
        { name: "audio", files: 19, bytes: 425_000_000 },
        { name: "image", files: 24, bytes: 62_000_000 },
      ],
  sites: fakeEmpty
    ? []
    : [
        { name: "youtube", files: 70, bytes: 5_000_000_000 },
        { name: "vimeo", files: 6, bytes: 440_000_000 },
        { name: "soundcloud", files: 4, bytes: 85_000_000 },
      ],
  days: fakeDays,
};

/** What a fake link holds, by its address. */
export function fakeLinkKind(url: string): "video" | "playlist" | "mix" | "channel" | "profile" {
  if (/x\.com|instagram\.com|bsky\.app/.test(url)) return "profile";
  if (/\/@/.test(url)) return "channel";
  if (/list=RD/.test(url)) return "mix";
  return /playlist|list=/.test(url) ? "playlist" : "video";
}

/** A fake job: reports `total` bytes (or with `items`, files) over about two seconds, then ends with `failed` or
 * done. */
export function fakeJob(id: number, total: number, onEnd: (failed: boolean) => void, failed = false, items = false) {
  let done = 0;
  const timer = setInterval(
    () => {
      if (fakeStopped.has(id)) {
        clearInterval(timer);
        fakeFinished.forEach((send) => void send({ id, result: { state: "cancelled" } }));
        return;
      }
      done = Math.min(total, items ? done + 1 : done + total / 16);
      const report: JobProgress = items
        ? { id, phase: "items", done, total, speed: null, eta: null, message: `File ${done}.jpg` }
        : { id, phase: "download", done, total, speed: 8e6, eta: 2, message: null };
      fakeProgress.forEach((send) => void send(report));
      if (done < total) return;
      clearInterval(timer);
      onEnd(failed);
      const result = failed
        ? { state: "failed" as const, message: "Download failed: connection reset" }
        : { state: "done" as const };
      fakeFinished.forEach((send) => void send({ id, result }));
    },
    items ? 60 : 120,
  );
}

/** A colored picture standing in for a thumbnail. */
export function fakePicture(hue: number, width = 320, height = 180): string {
  return `data:image/svg+xml,${encodeURIComponent(
    `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${width} ${height}"><rect width="${width}" height="${height}" fill="hsl(${hue} 35% 42%)"/><circle cx="${width * 0.7}" cy="${height * 0.35}" r="${height * 0.15}" fill="hsl(${hue} 50% 75%)"/></svg>`,
  )}`;
}

export const fakeGallery: GalleryItem[] = fakeEmpty
  ? []
  : Array.from({ length: 104 }, (_, index) => {
      const kind = index % 5 === 0 ? "audio" : index % 3 === 0 ? "image" : "video";
      const site = ["youtube", "vimeo", "soundcloud", "bluesky"][index % 4];
      const ext = { video: "mp4", audio: "mp3", image: "jpg" }[kind];
      const folder = { video: "Videos/NASA", audio: "Audio/Lofi Girl", image: "Images/bluesky" }[kind];
      const name = `${kind === "image" ? "Photo" : kind === "audio" ? "Track" : "Clip"} ${104 - index}.${ext}`;
      return {
        path: `${folder}/${name}`,
        name,
        kind,
        site,
        user: null,
        bytes: (index + 1) * 3_100_000,
        duration: kind === "image" ? null : 30 + index * 7,
        downloadedAt: Math.floor(Date.now() / 1000) - index * 5400,
        view: ({ video: "video", audio: "audio", image: "picture" } as const)[kind],
      };
    });

/** Every folder that holds one of `items`, with its count of files, subfolders' included. */
export function fakeFolders(items: GalleryItem[]): GalleryFolder[] {
  const counts = new Map<string, number>();
  for (const item of items) {
    const parts = item.path.split("/").slice(0, -1);
    parts.forEach((_, at) => {
      const path = parts.slice(0, at + 1).join("/");
      counts.set(path, (counts.get(path) ?? 0) + 1);
    });
  }
  return [...counts].map(([path, files]) => ({ path, files })).sort((a, b) => a.path.localeCompare(b.path));
}

export const fakePacks: Packs = {
  packs: [
    {
      id: "general",
      name: "General",
      group: "Photos",
      web: false,
      sites: [
        "wikipedia.org",
        "commons.wikimedia.org",
        "flickr.com",
        "unsplash.com",
        "pexels.com",
        "pixabay.com",
        "deviantart.com",
        "tumblr.com",
        "wallhaven.cc",
        "reddit.com",
        "pinterest.com",
      ],
      off: [],
      builtIn: true,
    },
    {
      id: "wallpapers",
      name: "Wallpapers",
      group: "Photos",
      web: false,
      sites: [
        "wallhaven.cc",
        "wallpapercave.com",
        "wallpaperaccess.com",
        "alphacoders.com",
        "unsplash.com",
        "pexels.com",
        "reddit.com",
      ],
      off: [],
      builtIn: true,
    },
    {
      id: "stock",
      name: "Free stock photos",
      group: "Photos",
      web: false,
      sites: [
        "unsplash.com",
        "pexels.com",
        "pixabay.com",
        "stocksnap.io",
        "kaboompics.com",
        "rawpixel.com",
        "commons.wikimedia.org",
      ],
      off: [],
      builtIn: true,
    },
    {
      id: "people",
      name: "People",
      group: "Entertainment",
      web: false,
      sites: [
        "wikipedia.org",
        "commons.wikimedia.org",
        "flickr.com",
        "imdb.com",
        "pinterest.com",
        "tumblr.com",
        "reddit.com",
      ],
      off: [],
      builtIn: true,
    },
    {
      id: "movies",
      name: "Movies and TV",
      group: "Entertainment",
      web: false,
      sites: ["imdb.com", "film-grab.com", "fandom.com", "wikipedia.org", "reddit.com"],
      off: [],
      builtIn: true,
    },
    {
      id: "games",
      name: "Games",
      group: "Entertainment",
      web: false,
      sites: [
        "fandom.com",
        "wikipedia.org",
        "steamgriddb.com",
        "mobygames.com",
        "igdb.com",
        "artstation.com",
        "deviantart.com",
        "steamcommunity.com",
        "reddit.com",
      ],
      off: [],
      builtIn: true,
    },
    {
      id: "anime",
      name: "Anime and manga",
      group: "Entertainment",
      web: false,
      sites: [
        "zerochan.net",
        "safebooru.org",
        "myanimelist.net",
        "anilist.co",
        "fandom.com",
        "wallhaven.cc",
        "pixiv.net",
        "reddit.com",
      ],
      off: [],
      builtIn: true,
    },
    {
      id: "memes",
      name: "Memes and GIFs",
      group: "Entertainment",
      web: false,
      sites: ["knowyourmeme.com", "imgflip.com", "tenor.com", "giphy.com", "reddit.com"],
      off: [],
      builtIn: true,
    },
    {
      id: "art",
      name: "Art styles",
      group: "Art and design",
      web: false,
      sites: [
        "artstation.com",
        "deviantart.com",
        "behance.net",
        "dribbble.com",
        "pixiv.net",
        "wikiart.org",
        "pinterest.com",
        "tumblr.com",
      ],
      off: [],
      builtIn: true,
    },
    {
      id: "museums",
      name: "Museum art",
      group: "Art and design",
      web: false,
      sites: [
        "nga.gov",
        "metmuseum.org",
        "artic.edu",
        "getty.edu",
        "rijksmuseum.nl",
        "wikiart.org",
        "artvee.com",
        "commons.wikimedia.org",
      ],
      off: [],
      builtIn: true,
    },
    {
      id: "architecture",
      name: "Architecture and interiors",
      group: "Art and design",
      web: false,
      sites: ["archdaily.com", "dezeen.com", "houzz.com", "architecturaldigest.com", "pinterest.com", "unsplash.com"],
      off: [],
      builtIn: true,
    },
    {
      id: "nature",
      name: "Nature and animals",
      group: "Science and history",
      web: false,
      sites: [
        "inaturalist.org",
        "flickr.com",
        "commons.wikimedia.org",
        "wikipedia.org",
        "pixabay.com",
        "unsplash.com",
        "pexels.com",
      ],
      off: [],
      builtIn: true,
    },
    {
      id: "space",
      name: "Space",
      group: "Science and history",
      web: false,
      sites: [
        "nasa.gov",
        "esawebb.org",
        "esahubble.org",
        "esa.int",
        "flickr.com",
        "commons.wikimedia.org",
        "wikipedia.org",
      ],
      off: [],
      builtIn: true,
    },
    {
      id: "history",
      name: "History and archives",
      group: "Science and history",
      web: false,
      sites: [
        "loc.gov",
        "nypl.org",
        "europeana.eu",
        "si.edu",
        "archive.org",
        "rawpixel.com",
        "commons.wikimedia.org",
        "flickr.com",
      ],
      off: [],
      builtIn: true,
    },
  ],
  problems: flags.has("packProblem") ? ["mine.toml: unknown field `site`, expected one of `name`, `web`, `sites`"] : [],
};
/** Library files the fake trash took. */
export const fakeTrashed = new Set<string>();

/** The fake gallery's pictures in groups of two or three, the first (largest) one best. */
export function fakeDuplicates(): Picture[][] {
  const pictures = fakeGallery.filter((item) => item.kind === "image" && !fakeTrashed.has(item.path));
  const groups: Picture[][] = [];
  for (let at = 0; at + 1 < pictures.length && groups.length < 4; at += 3) {
    groups.push(
      pictures.slice(at, at + (groups.length % 2 ? 2 : 3)).map((item, index) => ({
        path: item.path,
        name: item.name,
        width: 1600 >> index,
        height: 1000 >> index,
        bytes: item.bytes >> index,
        format: index === 0 ? "PNG" : "JPG",
        quality: index === 0 ? null : 92 - index * 10,
      })),
    );
  }
  return groups;
}

/** A page as a scan finds it: a video, a stream, audio, images (two of them tiny, under the default smallest size)
 * and a file. */
export function fakePage(url: string): PageScan {
  const base = "https://cdn.example.com/launch";
  return {
    url,
    title: "Launch day",
    kept: {},
    finds: [
      { url: `${base}/trailer.mp4`, kind: "video", name: "trailer.mp4", size: 48_200_000 },
      { url: `${base}/hls/master.m3u8`, kind: "stream", name: "master.m3u8", size: null },
      { url: `${base}/theme.mp3`, kind: "audio", name: "theme.mp3", size: 3_900_000 },
      ...["hero.jpg", "crew-1.jpg", "crew-2.jpg", "rocket.webp", "patch.png", "logo.svg"].map((name, index) => ({
        url: `${base}/img/${name}`,
        kind: "image" as const,
        name,
        size: index === 5 ? null : 180_000 * (index + 1),
      })),
      { url: `${base}/press-kit.pdf`, kind: "other", name: "press-kit.pdf", size: 2_400_000 },
      { url: `${base}/img/pixel.gif`, kind: "image", name: "pixel.gif", size: 43 },
      { url: `${base}/img/icon-16.png`, kind: "image", name: "icon-16.png", size: 900 },
      // `?bigPage`: a page of 1300 images, to try grabbing a lot at once.
      ...(flags.has("bigPage")
        ? Array.from({ length: 1300 }, (_, n) => ({
            url: `${base}/gallery/${n}.jpg`,
            kind: "image" as const,
            name: `photo-${n}.jpg`,
            size: 40_000 + n * 97,
          }))
        : []),
    ],
    pages: 1,
  };
}

/** `count` fake videos of a list: `Video 1`, `Video 2`… */
export function fakeListEntries(count: number, title: string, path: string): ListEntry[] {
  return Array.from({ length: count }, (_, n) => ({
    url: `https://www.youtube.com/${path}${n + 1}`,
    title: `${title}${n + 1}`,
    duration: path.startsWith("shorts") ? null : 45 + ((n * 97) % 1800),
    views: ((n * 7919) % 97) * 1300 + 400,
    tall: path.startsWith("shorts"),
  }));
}

/** What checking `url` finds: a video, a playlist (40 videos), a Mix (450, no count), a channel or a profile. */
export function fakeProbe(url: string): Probe {
  const one = { count: null, more: false, files: false, items: [], sections: [], entries: [], video: null };
  const video = { url, channel: "jawed", duration: 19, site: "youtube", thumbnail: fakeThumbnail };
  switch (fakeLinkKind(url)) {
    case "video":
      return { ...video, ...one, title: "Me at the zoo", heights: [1080, 720, 480, 240] };
    case "mix":
      return {
        ...video,
        title: "Mix - Sleeping With Sirens - Forever/Always",
        channel: null,
        duration: null,
        heights: [2160, 1440, 1080, 720, 480, 360],
        count: 450,
        more: false,
        files: false,
        items: [],
        sections: [],
        entries: fakeListEntries(450, "Mix video ", "watch?v=mx"),
        video: url.replace(/&list=[^&]*/, ""),
      };
    case "playlist":
      return {
        ...video,
        title: "Pygame Programming Tutorials",
        channel: "Tech With Tim",
        duration: null,
        heights: [2160, 1440, 1080, 720, 480, 360],
        count: 40,
        more: false,
        files: false,
        items: ["Pygame Tutorial #1 - Basic Movement", "Pygame Tutorial #2 - Jumping", "Pygame Tutorial #3 - Sprites"],
        sections: [],
        entries: fakeListEntries(40, "Pygame Tutorial #", "watch?v=pg"),
        video: url.includes("v=") ? url.replace(/&list=[^&]*/, "") : null,
      };
    case "channel":
      return {
        ...video,
        title: "GDULA",
        channel: "GDULA",
        duration: null,
        heights: [2160, 1440, 1080, 720, 480, 360],
        count: 22,
        more: false,
        files: false,
        items: ["GDULA - TOM FORD", "GDULA - KOSMOS", "GDULA - NOCE"],
        sections: [
          {
            url: `${url}/videos`,
            title: "Videos",
            count: 6,
            more: false,
            entries: fakeListEntries(6, "Video ", "watch?v=v"),
          },
          {
            url: `${url}/shorts`,
            title: "Shorts",
            count: 16,
            more: false,
            entries: fakeListEntries(16, "Short ", "shorts/s"),
          },
        ],
        entries: [],
        video: null,
      };
    case "profile":
      return {
        url,
        title: "bsky.app",
        channel: null,
        duration: null,
        site: "bluesky",
        thumbnail: fakeThumbnail,
        heights: [],
        count: 50,
        more: true,
        files: true,
        items: ["3lmxqzg2b4c2a_1.jpeg", "3lmxqzg2b4c2a_2.jpeg", "3llwpk7rjvs2x_1.mp4"],
        sections: [],
        entries: [],
        video: null,
      };
  }
}

/** `probe`'s first `share` of each list, as a check shows it while listing; stopped, what was listed is the list. */
/** A mirror's images and videos, as its crawl lists them: 8 clips and 120 pictures, their sizes unknown. */
export function fakeMirrorFinds(url: string): Find[] {
  const base = new URL(url).origin;
  const listed = (kind: "image" | "video", name: string, count: number) =>
    Array.from({ length: count }, (_, n) => ({
      url: `${base}/media/${name}-${n + 1}.${kind === "video" ? "mp4" : "jpg"}`,
      kind,
      name: `${name}-${n + 1}.${kind === "video" ? "mp4" : "jpg"}`,
      size: null,
    }));
  return [...listed("video", "clip", 8), ...listed("image", "photo", 120)];
}

export function fakePart(probe: Probe, share: number, stopped: boolean): Probe {
  const cut = <T extends { entries: ListEntry[]; count: number | null }>(list: T): T => {
    const entries = list.entries.slice(0, Math.max(1, Math.ceil(list.entries.length * share)));
    return { ...list, entries, count: stopped ? entries.length : list.count };
  };
  const sections = probe.sections.map((section) => ({ ...cut(section), count: cut(section).count ?? 0 }));
  const part = cut(probe);
  const count = sections.length > 0 ? sections.reduce((sum, section) => sum + section.count, 0) : part.count;
  // A Mix has no count until it's listed: what's listed so far.
  return { ...part, sections, count: probe.title.startsWith("Mix") ? part.entries.length : count };
}

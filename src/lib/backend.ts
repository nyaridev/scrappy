// Backend: the only file that talks to Rust (invoke and listen). Outside the app (`pnpm dev` in a browser) it
// answers with fake data, so every view can be previewed.
// Preview flags: `?setup` (first run), `?notEmpty` (first run, the picked folder isn't empty), `?missing` (the library
// folder is gone), `?tools` (the tools step: nothing installed), `?toolFails` (the tools step, Deno's download fails),
// `?outdated` (the tools step: Scrappy's yt-dlp and a system FFmpeg are older than the pins), `?probeFails` (a
// link check fails), `?empty` (a library with no downloads yet, no signed-in sites), `?signIn` (a link check needs a
// sign-in),
// links with `playlist` or `/@` (a 40-video playlist), links to x.com, instagram.com or bsky.app (a profile with
// 50+ files), `?scanFails` (Grab's scan finds no browser), `?handed` (the extension handed a page to Grab at start),
// `?overlay` (macOS title bar), `?native` (system title bar), `?packProblem` (a user's Hunt pack can't be read), `?noHost` (no scrappy-host beside the app). The default is a ready library, every tool installed and
// the custom title bar.

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { AppInfo } from "./types/AppInfo";
import type { CrawlRequest } from "./types/CrawlRequest";
import type { CrawlStarted } from "./types/CrawlStarted";
import type { DownloadEntry } from "./types/DownloadEntry";
import type { DownloadRequest } from "./types/DownloadRequest";
import type { DuplicateRequest } from "./types/DuplicateRequest";
import type { FolderCheck } from "./types/FolderCheck";
import type { GalleryItem } from "./types/GalleryItem";
import type { GalleryPage } from "./types/GalleryPage";
import type { GalleryQuery } from "./types/GalleryQuery";
import type { GrabRequest } from "./types/GrabRequest";
import type { HostStatus } from "./types/HostStatus";
import type { HuntRequest } from "./types/HuntRequest";
import type { HuntStarted } from "./types/HuntStarted";
import type { JobFinished } from "./types/JobFinished";
import type { JobProgress } from "./types/JobProgress";
import type { LibraryStatus } from "./types/LibraryStatus";
import type { Look } from "./types/Look";
import type { Packs } from "./types/Packs";
import type { PageScan } from "./types/PageScan";
import type { Picked } from "./types/Picked";
import type { Picture } from "./types/Picture";
import type { PresetInfo } from "./types/PresetInfo";
import type { Probe } from "./types/Probe";
import type { RecentDownload } from "./types/RecentDownload";
import type { RememberedSite } from "./types/RememberedSite";
import type { Settings } from "./types/Settings";
import type { Stats } from "./types/Stats";
import type { Tool } from "./types/Tool";
import type { ToolJob } from "./types/ToolJob";
import type { ToolStatus } from "./types/ToolStatus";
import type { WindowFrame } from "./types/WindowFrame";

// ── Fake backend ──────────────────────────
const inTauri = "__TAURI_INTERNALS__" in window;
const flags = new URLSearchParams(window.location.search);
const fakeRoot = "D:/Scrappy";
const fakeFrame: WindowFrame = flags.has("native") ? "native" : flags.has("overlay") ? "overlay" : "custom";
let fakeLibrary: LibraryStatus =
  flags.has("setup") || flags.has("notEmpty")
    ? { state: "notSet" }
    : flags.has("missing")
      ? { state: "missing", root: "E:/Scrappy" }
      : { state: "ready", root: fakeRoot };
let fakeSettings: Settings = {
  libraryRoot: fakeLibrary.state === "notSet" ? null : fakeLibrary.root,
  look: { nativeFrame: fakeFrame === "native", liquidBackground: true },
};

/** Rust's answers, roughly: rejects with a plain message, as `invoke` does. */
async function fakeCheck(input: string): Promise<FolderCheck> {
  const path = input.trim();
  if (!path) return Promise.reject("Enter a folder.");
  if (!/^([a-z]:[\\/]|\/)/i.test(path)) return Promise.reject("Not a full path.");
  return { path, exists: true, writable: true, empty: !flags.has("notEmpty"), library: false };
}

const toolsMissing = ["setup", "notEmpty", "missing", "tools", "toolFails"].some((flag) => flags.has(flag));
const pins: Record<Tool, string> = { "yt-dlp": "2026.10.26", deno: "2.9.8", "gallery-dl": "1.33.0", ffmpeg: "9.0.2" };
let fakeTools: ToolStatus[] = (Object.keys(pins) as Tool[]).map((tool) => ({
  tool,
  source: toolsMissing ? "missing" : "scrappy",
  version: toolsMissing ? null : pins[tool],
  pinnedVersion: pins[tool],
  outdated: false,
  downloadable: true,
  job: null,
}));
if (flags.has("outdated")) {
  fakeTools[0] = { ...fakeTools[0], version: "2025.01.01", outdated: true };
  fakeTools[3] = { ...fakeTools[3], source: "system", version: "7.1-full_build", outdated: true };
}
let fakeHandedTaken = false;
const fakeProgress = new Set<(progress: JobProgress) => void>();
const fakeFinished = new Set<(finished: JobFinished) => void>();
let fakeJobId = 0;

const fakePresets: PresetInfo[] = [
  { id: "editor-mp4", name: "MP4 (H.264)", audio: false },
  { id: "original", name: "MKV (original)", audio: false },
  { id: "hevc-mp4", name: "MP4 (HEVC)", audio: false },
  { id: "prores-mov", name: "MOV (ProRes)", audio: false },
  { id: "mp3", name: "MP3 (320 kbps)", audio: true },
  { id: "m4a", name: "M4A (AAC)", audio: true },
  { id: "flac", name: "FLAC", audio: true },
  { id: "wav", name: "WAV", audio: true },
];
let fakeDownloads: DownloadEntry[] = [];
const fakeThumbnail = `data:image/svg+xml,${encodeURIComponent(
  '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 320 180"><defs><linearGradient id="g"><stop stop-color="#3a5f4a"/><stop offset="1" stop-color="#8fae6e"/></linearGradient></defs><rect width="320" height="180" fill="url(#g)"/></svg>',
)}`;

const fakeEmpty = flags.has("empty");
const fakeRecent: RecentDownload[] = fakeEmpty
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
const fakeStats: Stats = {
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
function fakeLinkKind(url: string): "video" | "playlist" | "profile" {
  if (/x\.com|instagram\.com|bsky\.app/.test(url)) return "profile";
  return /playlist|\/@/.test(url) ? "playlist" : "video";
}

/** A fake job: reports `total` bytes (or with `items`, files) over about two seconds, then ends with `failed` or
 * done. */
function fakeJob(id: number, total: number, onEnd: (failed: boolean) => void, failed = false, items = false) {
  let done = 0;
  const timer = setInterval(
    () => {
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
function fakePicture(hue: number, width = 320, height = 180): string {
  return `data:image/svg+xml,${encodeURIComponent(
    `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${width} ${height}"><rect width="${width}" height="${height}" fill="hsl(${hue} 35% 42%)"/><circle cx="${width * 0.7}" cy="${height * 0.35}" r="${height * 0.15}" fill="hsl(${hue} 50% 75%)"/></svg>`,
  )}`;
}

const fakeGallery: GalleryItem[] = fakeEmpty
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
        missing: index === 7,
      };
    });

/** Library files the fake trash took. */
const fakePacks: Packs = {
  packs: [
    { id: "general", name: "General", web: true, sites: [] },
    {
      id: "games",
      name: "Games",
      web: true,
      sites: ["fandom.com", "wikipedia.org", "artstation.com", "deviantart.com"],
    },
    {
      id: "art",
      name: "Art styles",
      web: true,
      sites: ["artstation.com", "deviantart.com", "behance.net", "wikiart.org"],
    },
    { id: "people", name: "People", web: true, sites: ["wikipedia.org", "commons.wikimedia.org", "flickr.com"] },
  ],
  problems: flags.has("packProblem") ? ["mine.toml: unknown field `site`, expected one of `name`, `web`, `sites`"] : [],
};
let fakeHost: HostStatus = { connected: [], available: !flags.has("noHost") };
const fakeTrashed = new Set<string>();
const fakeFound = new Map<number, Picture[][]>();

/** The fake gallery's pictures in groups of two or three, the first (largest) one best. */
function fakeDuplicates(): Picture[][] {
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

/** A page as a scan finds it: a video, a stream, audio, images and a file. */
function fakePage(url: string): PageScan {
  const base = "https://cdn.example.com/launch";
  return {
    url,
    title: "Launch day",
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
    ],
  };
}

/** Fake installs: each missing tool "downloads" over about two seconds. */
function fakeInstall(): ToolJob[] {
  return fakeTools
    .filter((status) => status.source === "missing" || status.outdated || status.job !== null)
    .map((status) => {
      if (status.job !== null) return { tool: status.tool, id: status.job };
      const id = ++fakeJobId;
      const fails = flags.has("toolFails") && status.tool === "deno";
      fakeJob(
        id,
        40_000_000,
        (failed) => {
          fakeTools = fakeTools.map((s) =>
            s.tool === status.tool
              ? { ...s, job: null, ...(failed ? {} : { source: "scrappy", version: s.pinnedVersion, outdated: false }) }
              : s,
          );
        },
        fails,
      );
      fakeTools = fakeTools.map((s) => (s.tool === status.tool ? { ...s, job: id } : s));
      return { tool: status.tool, id };
    });
}

// ── App ───────────────────────────────────
/** The running version, platform and window frame. */
export async function appInfo(): Promise<AppInfo> {
  return inTauri ? invoke("app_info") : { version: "0.1.0", os: "browser preview", frame: fakeFrame };
}

// ── Library ───────────────────────────────
/** Where the library stands: not set up yet, ready, or missing. */
export async function libraryStatus(): Promise<LibraryStatus> {
  return inTauri ? invoke("library_status") : fakeLibrary;
}

/** Opens the system folder picker; Rust checks the folder. `null` when the user cancels. */
export async function pickFolder(): Promise<FolderCheck | null> {
  return inTauri ? invoke("pick_folder") : fakeCheck(fakeRoot);
}

/** Checks a folder path the user typed. Rejects with Rust's message when it isn't a usable full path. */
export async function checkFolder(path: string): Promise<FolderCheck> {
  return inTauri ? invoke("check_folder", { path }) : fakeCheck(path);
}

/** Sets up the library at `path`, creating the folder when it isn't there. */
export async function setupLibrary(path: string): Promise<LibraryStatus> {
  if (inTauri) return invoke("setup_library", { path });
  fakeLibrary = { state: "ready", root: path };
  fakeSettings = { ...fakeSettings, libraryRoot: path };
  return fakeLibrary;
}

// ── Tools ─────────────────────────────────
/** Every tool, where it is, its version and any install running. */
export async function toolsStatus(): Promise<ToolStatus[]> {
  return inTauri ? invoke("tools_status") : fakeTools;
}

/** Starts installing the missing tools; follow the returned jobs with `onJobProgress` and `onJobFinished`. */
export async function installTools(): Promise<ToolJob[]> {
  return inTauri ? invoke("install_tools") : fakeInstall();
}

// ── Jobs ──────────────────────────────────
/** Asks a job to stop. */
export async function cancelJob(id: number): Promise<boolean> {
  return inTauri ? invoke("cancel_job", { id }) : false;
}

/** Calls `handler` with every job's progress. Returns the unsubscribe. */
export function onJobProgress(handler: (progress: JobProgress) => void): () => void {
  if (!inTauri) {
    fakeProgress.add(handler);
    return () => void fakeProgress.delete(handler);
  }
  const unlisten = listen<JobProgress>("job-progress", (event) => handler(event.payload));
  return () => void unlisten.then((stop) => stop());
}

/** Calls `handler` when any job ends. Returns the unsubscribe. */
export function onJobFinished(handler: (finished: JobFinished) => void): () => void {
  if (!inTauri) {
    fakeFinished.add(handler);
    return () => void fakeFinished.delete(handler);
  }
  const unlisten = listen<JobFinished>("job-finished", (event) => handler(event.payload));
  return () => void unlisten.then((stop) => stop());
}

// ── Media ─────────────────────────────────
/** A link's details: title, channel, length, thumbnail and the video heights on offer; for a playlist or profile, its
 * size and first entries. */
export async function probeUrl(url: string): Promise<Probe> {
  if (inTauri) return invoke("probe_url", { url });
  await new Promise((resolve) => setTimeout(resolve, 600));
  if (flags.has("probeFails") || !url.startsWith("http"))
    return Promise.reject("Video unavailable. This video is private");
  if (flags.has("signIn"))
    return Promise.reject(
      "youtube.com needs you signed in. Send the page from the Scrappy extension to share your sign-in.",
    );
  const one = { count: null, more: false, files: false, items: [] };
  const video = { url, channel: "jawed", duration: 19, site: "youtube", thumbnail: fakeThumbnail };
  switch (fakeLinkKind(url)) {
    case "video":
      return { ...video, ...one, title: "Me at the zoo", heights: [1080, 720, 480, 240] };
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
      };
  }
}

/** The output presets, the default first. */
export async function presets(): Promise<PresetInfo[]> {
  return inTauri ? invoke("presets") : fakePresets;
}

/** Queues a download; follow its progress by the entry's id. */
export async function startDownload(request: DownloadRequest): Promise<DownloadEntry> {
  if (inTauri) return invoke("start_download", { request });
  const id = ++fakeJobId;
  const kind = fakeLinkKind(request.url);
  const preset = fakePresets.find((p) => p.id === request.preset)?.name ?? request.preset;
  const entry: DownloadEntry = {
    id,
    url: request.url,
    title: { video: "Me at the zoo", playlist: "Pygame Programming Tutorials", profile: "bsky.app" }[kind],
    preset: kind === "profile" ? "Original files" : preset,
    state: "queued",
    thumbnail: fakeThumbnail,
    file: null,
    files: null,
    skipped: null,
    error: null,
  };
  const saved = {
    video: { file: "Videos/jawed/Me at the zoo [jNQXAC9IVRw].mp4", files: null, skipped: null },
    playlist: {
      file: "Videos/Tech With Tim/Pygame Programming Tutorials",
      files: (request.limit ?? 40) - 2,
      skipped: 2,
    },
    profile: { file: "Social/bluesky/bsky.app", files: request.limit ?? 50, skipped: 0 },
  }[kind];
  const total = kind === "video" ? 30_000_000 : (request.limit ?? (kind === "playlist" ? 40 : 50));
  fakeDownloads = [...fakeDownloads, entry];
  fakeJob(
    id,
    total,
    (failed) => {
      fakeDownloads = fakeDownloads.map((d) =>
        d.id === id
          ? failed
            ? { ...d, state: "failed", error: "Download failed: connection reset" }
            : { ...d, state: "done", ...saved }
          : d,
      );
    },
    false,
    kind !== "video",
  );
  return entry;
}

/** This session's downloads. */
export async function downloads(): Promise<DownloadEntry[]> {
  return inTauri ? invoke("downloads") : fakeDownloads;
}

// ── Grab ──────────────────────────────────
/** Scans a page in a headless browser and lists what can be downloaded; takes a while. */
export async function scanPage(url: string): Promise<PageScan> {
  if (inTauri) return invoke("scan_page", { url });
  await new Promise((resolve) => setTimeout(resolve, 1500));
  if (flags.has("scanFails")) return Promise.reject("Scanning needs Chrome, Brave or Edge installed.");
  if (!url.startsWith("http")) return Promise.reject("Not a link.");
  return fakePage(url);
}

/** Queues the picked finds into the library; follow each by its id. */
export async function grabFinds(request: GrabRequest): Promise<Picked[]> {
  if (inTauri) return invoke("grab_finds", { request });
  return request.finds.map((pick) => {
    const id = ++fakeJobId;
    const entry: DownloadEntry = {
      id,
      url: pick.url,
      title: pick.name,
      preset: pick.kind === "stream" ? "MP4 (H.264)" : "Original file",
      state: "queued",
      thumbnail: null,
      file: null,
      files: null,
      skipped: null,
      error: null,
    };
    fakeDownloads = [...fakeDownloads, entry];
    fakeJob(id, 12_000_000, () => {
      const file = `Grabs/example.com/2026-11-10/${pick.name}`;
      fakeDownloads = fakeDownloads.map((d) => (d.id === id ? { ...d, state: "done", file } : d));
    });
    return { url: pick.url, id, error: null };
  });
}

/** The page the extension handed to Grab, once. */
export async function handedPage(): Promise<PageScan | null> {
  if (inTauri) return invoke("handed_page");
  if (!flags.has("handed") || fakeHandedTaken) return null;
  fakeHandedTaken = true;
  return fakePage("https://example.com/launch");
}

// ── Scrape ────────────────────────────────
/** Starts a crawl of a site for its files; follow it by the returned id. */
export async function crawlSite(request: CrawlRequest): Promise<CrawlStarted> {
  if (inTauri) return invoke("crawl_site", { request });
  if (!request.url.startsWith("http")) return Promise.reject("Not a link.");
  const id = ++fakeJobId;
  const host = new URL(request.url).host.replace(/^www\./, "");
  const folder = request.mode === "files" ? `Images/${host}` : `Sites/${host}`;
  const open = { files: null, mirror: `${folder}/index.html`, page: `${folder}/_pages/index.html` }[request.mode];
  const total = request.mode === "page" ? 1 : request.files;
  let pages = 0;
  let files = 0;
  const timer = setInterval(() => {
    pages = Math.min(request.pages, pages + 1);
    files = Math.min(total, files + 3);
    const message = request.mode === "page" ? null : pages === 1 ? "1 page" : `${pages} pages`;
    fakeProgress.forEach(
      (send) => void send({ id, phase: "items", done: files, total, speed: null, eta: null, message }),
    );
    if (files < total && pages < request.pages) return;
    clearInterval(timer);
    fakeFinished.forEach((send) => void send({ id, result: { state: "done" } }));
  }, 150);
  return { id, folder, open };
}

// ── Hunt ──────────────────────────────────
/** The source packs, and the user's packs that couldn't be read. */
export async function huntPacks(): Promise<Packs> {
  return inTauri ? invoke("hunt_packs") : fakePacks;
}

/** Starts a hunt; follow it by the returned id. */
export async function startHunt(request: HuntRequest): Promise<HuntStarted> {
  if (inTauri) return invoke("start_hunt", { request });
  const id = ++fakeJobId;
  const places = [...(request.web ? ["the web"] : []), ...request.sites];
  let saved = 0;
  const timer = setInterval(() => {
    saved = Math.min(request.files, saved + 4);
    const message = saved < 8 ? `Searching ${places[0]}` : ["fandom.com", "wikipedia.org", "gameforge.com"][saved % 3];
    fakeProgress.forEach(
      (send) => void send({ id, phase: "items", done: saved, total: request.files, speed: null, eta: null, message }),
    );
    if (saved < request.files) return;
    clearInterval(timer);
    fakeFinished.forEach((send) => void send({ id, result: { state: "done" } }));
  }, 120);
  return { id, folder: `Images/${request.query.trim()}` };
}

/** Opens the folder for the user's own Hunt packs, making it first. */
export async function openPacksFolder(): Promise<void> {
  if (inTauri) return invoke("open_packs_folder");
}

// ── Browser extension ─────────────────────
/** Which browsers reach this app through the extension. */
export async function hostStatus(): Promise<HostStatus> {
  return inTauri ? invoke("host_status") : fakeHost;
}

/** Connects every installed browser to this app. */
export async function connectBrowsers(): Promise<HostStatus> {
  if (inTauri) return invoke("connect_browsers");
  fakeHost = { ...fakeHost, connected: ["Chrome", "Edge", "Firefox"] };
  return fakeHost;
}

/** Disconnects every browser. */
export async function disconnectBrowsers(): Promise<HostStatus> {
  if (inTauri) return invoke("disconnect_browsers");
  fakeHost = { ...fakeHost, connected: [] };
  return fakeHost;
}

// ── Library history ───────────────────────
/** The library's totals, per kind and site, and the last 30 days. */
export async function libraryStats(): Promise<Stats> {
  return inTauri ? invoke("library_stats") : fakeStats;
}

/** The latest `limit` downloads, newest first. */
export async function recentDownloads(limit: number): Promise<RecentDownload[]> {
  return inTauri ? invoke("recent_downloads", { limit }) : fakeRecent.slice(0, limit);
}

/** One page of the gallery. */
export async function galleryPage(query: GalleryQuery): Promise<GalleryPage> {
  if (inTauri) return invoke("gallery_page", { query });
  await new Promise((resolve) => setTimeout(resolve, 150));
  const search = query.search?.trim().toLowerCase() ?? "";
  const matching = fakeGallery
    .filter((item) => !fakeTrashed.has(item.path))
    .filter((item) => (!query.kind || item.kind === query.kind) && (!query.site || item.site === query.site))
    .filter((item) => item.path.toLowerCase().includes(search));
  const sorted = [...matching].sort((a, b) => {
    switch (query.sort) {
      case "oldest":
        return a.downloadedAt - b.downloadedAt;
      case "largest":
        return b.bytes - a.bytes;
      case "name":
        return a.name.localeCompare(b.name);
      default:
        return b.downloadedAt - a.downloadedAt;
    }
  });
  return { items: sorted.slice(query.offset, query.offset + query.limit), total: sorted.length };
}

/** A library file's thumbnail as a `data:` URL; `null` when it has no picture. */
export async function thumbnail(path: string): Promise<string | null> {
  if (inTauri) return invoke("thumbnail", { path });
  await new Promise((resolve) => setTimeout(resolve, 200 + Math.random() * 400));
  return /\.mp3$/.test(path) ? null : fakePicture((path.length * 37) % 360);
}

/** A library picture whole, as a `data:` URL; `null` when the window can't show it (it opens in its app). */
export async function picture(path: string): Promise<string | null> {
  if (inTauri) return invoke("picture", { path });
  return /\.jpg$/.test(path) ? fakePicture((path.length * 37) % 360, 1600, 1000) : null;
}

/** Opens a library file in the system's default app for it. */
export async function openFile(path: string): Promise<void> {
  if (inTauri) return invoke("open_file", { path });
}

/** Starts a scan of the library for near-duplicate pictures; follow it by the returned id. */
export async function findDuplicates(request: DuplicateRequest): Promise<number> {
  if (inTauri) return invoke("find_duplicates", { request });
  const id = ++fakeJobId;
  fakeJob(id, 30, () => fakeFound.set(id, flags.has("empty") ? [] : fakeDuplicates()), false, true);
  return id;
}

/** The groups a finished duplicate scan found, best copy first; once. */
export async function duplicatesFound(id: number): Promise<Picture[][] | null> {
  if (inTauri) return invoke("duplicates_found", { id });
  const found = fakeFound.get(id) ?? null;
  fakeFound.delete(id);
  return found;
}

/** Moves library files to the system trash; returns how many moved. */
export async function trashFiles(paths: string[]): Promise<number> {
  if (inTauri) return invoke("trash_files", { paths });
  for (const path of paths) fakeTrashed.add(path);
  return paths.length;
}

/** Opens the system file manager with a library file selected. */
export async function showInFolder(path: string): Promise<void> {
  if (inTauri) return invoke("show_in_folder", { path });
}

// ── Sign-ins ──────────────────────────────
let fakeSites: RememberedSite[] = flags.has("empty")
  ? []
  : [
      { site: "x.com", cookies: 14, savedAt: Math.floor(Date.now() / 1000) - 86_400 },
      { site: "youtube.com", cookies: 31, savedAt: Math.floor(Date.now() / 1000) - 9 * 86_400 },
    ];

/** The sites whose sign-in is remembered. */
export async function signedInSites(): Promise<RememberedSite[]> {
  return inTauri ? invoke("signed_in_sites") : fakeSites;
}

/** Forgets a site's sign-in. */
export async function forgetSite(site: string): Promise<void> {
  if (inTauri) return invoke("forget_site", { site });
  fakeSites = fakeSites.filter((each) => each.site !== site);
}

// ── Settings ──────────────────────────────
/** The saved settings. */
export async function getSettings(): Promise<Settings> {
  return inTauri ? invoke("get_settings") : fakeSettings;
}

/** Saves a new look and returns the settings. */
export async function setLook(look: Look): Promise<Settings> {
  if (inTauri) return invoke("set_look", { look });
  fakeSettings = { ...fakeSettings, look };
  return fakeSettings;
}

// ── Window ────────────────────────────────
/** Minimizes the window. */
export function minimizeWindow(): void {
  if (inTauri) void getCurrentWindow().minimize();
}

/** Maximizes the window, or restores it when it's maximized. */
export function toggleMaximizeWindow(): void {
  if (inTauri) void getCurrentWindow().toggleMaximize();
}

/** Closes the window, which quits the app. */
export function closeWindow(): void {
  if (inTauri) void getCurrentWindow().close();
}

/** Calls `handler` each time the window gains focus. Returns the unsubscribe. */
export function onWindowFocus(handler: () => void): () => void {
  if (!inTauri) {
    window.addEventListener("focus", handler);
    return () => window.removeEventListener("focus", handler);
  }
  const unlisten = getCurrentWindow().onFocusChanged(({ payload: focused }) => focused && handler());
  return () => void unlisten.then((stop) => stop());
}

/** Calls `onChange` with whether the window is maximized, now and after every resize. Returns the unsubscribe. */
export function watchMaximized(onChange: (maximized: boolean) => void): () => void {
  if (!inTauri) return () => {};
  const current = getCurrentWindow();
  const update = () => void current.isMaximized().then(onChange);
  update();
  const unlisten = current.onResized(update);
  return () => void unlisten.then((stop) => stop());
}

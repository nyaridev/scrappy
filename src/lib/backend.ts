// Backend: the only file that talks to Rust (invoke and listen). Outside the app (`pnpm dev` in a browser) it
// answers with fake data, so every view can be previewed: the data and its preview flags are in `fakeData.ts`, the
// fake hunt in `fakeHunt.ts`; the fake state those answers change is kept here.

import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  fakeCheck,
  fakeDownloadsAtStart,
  fakeDuplicates,
  fakeFinished,
  fakeFolders,
  fakeFrame,
  fakeGallery,
  fakeJob,
  fakeLibraryAtStart,
  fakeLinkKind,
  fakeMirrorFinds,
  fakeOther,
  fakePacks,
  fakePage,
  fakePart,
  fakePicture,
  fakePresets,
  fakeProbe,
  fakeProgress,
  fakeRecent,
  fakeRoot,
  fakeSettingsAtStart,
  fakeStats,
  fakeStopped,
  fakeThumbnail,
  fakeToolsAtStart,
  fakeTrashed,
  flags,
} from "./fakeData";
import { fakeHunt } from "./fakeHunt";
import type { AppInfo } from "./types/AppInfo";
import type { CrawlRequest } from "./types/CrawlRequest";
import type { CrawlStarted } from "./types/CrawlStarted";
import type { DownloadEntry } from "./types/DownloadEntry";
import type { DownloadLimits } from "./types/DownloadLimits";
import type { DownloadRequest } from "./types/DownloadRequest";
import type { DownloadSource } from "./types/DownloadSource";
import type { DuplicateRequest } from "./types/DuplicateRequest";
import type { FindKind } from "./types/FindKind";
import type { FolderCheck } from "./types/FolderCheck";
import type { GalleryPage } from "./types/GalleryPage";
import type { GalleryQuery } from "./types/GalleryQuery";
import type { GrabRequest } from "./types/GrabRequest";
import type { HostStatus } from "./types/HostStatus";
import type { HuntRequest } from "./types/HuntRequest";
import type { HuntStarted } from "./types/HuntStarted";
import type { HuntStatus } from "./types/HuntStatus";
import type { JobFinished } from "./types/JobFinished";
import type { JobProgress } from "./types/JobProgress";
import type { LibraryStatus } from "./types/LibraryStatus";
import type { Look } from "./types/Look";
import type { MediaRequest } from "./types/MediaRequest";
import type { MirrorMedia } from "./types/MirrorMedia";
import type { Pack } from "./types/Pack";
import type { Packs } from "./types/Packs";
import type { PageScan } from "./types/PageScan";
import type { Picked } from "./types/Picked";
import type { Picture } from "./types/Picture";
import type { Player } from "./types/Player";
import type { Preferences } from "./types/Preferences";
import type { PresetInfo } from "./types/PresetInfo";
import type { Probe } from "./types/Probe";
import type { RecentDownload } from "./types/RecentDownload";
import type { RememberedSite } from "./types/RememberedSite";
import type { ScanRequest } from "./types/ScanRequest";
import type { Settings } from "./types/Settings";
import type { Stats } from "./types/Stats";
import type { ToolJob } from "./types/ToolJob";
import type { ToolStatus } from "./types/ToolStatus";
import type { UpdateStatus } from "./types/UpdateStatus";

// ── Fake backend ──────────────────────────
const inTauri = "__TAURI_INTERNALS__" in window;
let fakeLibrary = fakeLibraryAtStart;
let fakeExtensionFolder: string | null = null;
let fakeSettings = fakeSettingsAtStart;
let fakeTools = fakeToolsAtStart;
let fakeDownloads = fakeDownloadsAtStart;
let fakeHandedTaken = false;
// Above the kept fake downloads' ids.
let fakeJobId = 1000;
/** Set after the first fake duplicate scan: later ones end at once, as the app's do with every picture hashed. */
let fakeHashed = false;
/** The fake hunt's sources, read as they are now. */
let fakeHuntStatus: { id: number; read: () => HuntStatus } | null = null;
/** Fake scans: what each has found so far. */
const fakeScans = new Map<number, PageScan>();
const fakeChecks = new Map<number, Probe>();
const fakeMirrors = new Map<string, MirrorMedia>();
let fakeHost: HostStatus = { connected: [], available: !flags.has("noHost") };
const fakeFound = new Map<number, Picture[][]>();

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

// ── Updates ───────────────────────────────
// Outside the app: `?update` has a newer version to install, which "downloads" over two seconds.
let fakeUpdate: UpdateStatus = {
  current: "0.1.0",
  latest: flags.has("update") ? "0.2.0" : null,
  step: null,
  progress: null,
  error: null,
  canUpdate: true,
  checkedAt: null,
};
const fakeUpdateHandlers = new Set<(status: UpdateStatus) => void>();

function setFakeUpdate(change: Partial<UpdateStatus>) {
  fakeUpdate = { ...fakeUpdate, ...change };
  for (const handler of fakeUpdateHandlers) handler(fakeUpdate);
}

/** What Scrappy's updater knows now (ADR-0037). */
export async function updateStatus(): Promise<UpdateStatus> {
  return inTauri ? invoke("update_status") : fakeUpdate;
}

/** Looks for a newer Scrappy now. */
export async function checkForUpdate(): Promise<UpdateStatus> {
  if (inTauri) return invoke("check_for_update");
  setFakeUpdate({ step: "checking", error: null });
  await new Promise((resolve) => setTimeout(resolve, 800));
  setFakeUpdate({ step: null, checkedAt: Math.floor(Date.now() / 1000) });
  return fakeUpdate;
}

/** Installs the update a check found; Scrappy restarts into it. */
export async function installUpdate(): Promise<void> {
  if (inTauri) return invoke("install_update");
  for (let percent = 0; percent <= 100; percent += 10) {
    setFakeUpdate({ step: "downloading", progress: percent / 100 });
    await new Promise((resolve) => setTimeout(resolve, 200));
  }
  setFakeUpdate({ step: "installing", progress: null });
  await new Promise((resolve) => setTimeout(resolve, 800));
  setFakeUpdate({ current: fakeUpdate.latest ?? fakeUpdate.current, latest: null, step: null });
}

/** Calls `handler` with each change of the updater's status. */
export function onUpdateChanged(handler: (status: UpdateStatus) => void): () => void {
  if (!inTauri) {
    fakeUpdateHandlers.add(handler);
    return () => void fakeUpdateHandlers.delete(handler);
  }
  const unlisten = listen<UpdateStatus>("update-changed", (event) => handler(event.payload));
  return () => void unlisten.then((stop) => stop());
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

/** Starts moving the library to `path` as a job (ADR-0039); follow it with `onJobProgress` and `onJobFinished`. When
 * it's done, the library is at `path`. */
export async function moveLibrary(path: string): Promise<number> {
  if (inTauri) return invoke("move_library", { path });
  const id = ++fakeJobId;
  fakeJob(id, 2_400_000_000, (failed) => {
    if (failed) return;
    fakeLibrary = { state: "ready", root: path };
    fakeSettings = { ...fakeSettings, libraryRoot: path };
  });
  return id;
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
  if (inTauri) return invoke("cancel_job", { id });
  fakeStopped.add(id);
  return false;
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
/** Checks a link as a job (ADR-0030): its progress is the videos listed so far, and `listedLink` returns what it
 * found. A list shows from its first videos and grows; stopped, what was listed is the list. */
export async function listLink(url: string): Promise<number> {
  if (inTauri) return invoke("list_link", { url });
  const id = ++fakeJobId;
  const failure =
    flags.has("probeFails") || !url.startsWith("http") ? "Video unavailable. This video is private" : null;
  const signIn = "youtube.com needs you signed in. Send the page from the Scrappy extension to share your sign-in.";
  const error = flags.has("signIn") ? signIn : failure;
  const all = fakeProbe(url);
  fakeChecks.clear();
  // A list grows over four seconds, as a long one does; anything else shows when it's done.
  const steps = all.count != null && !all.files ? 8 : 1;
  let step = 0;
  const timer = setInterval(() => {
    step += 1;
    const stopped = fakeStopped.has(id);
    const ended = stopped || step >= steps;
    if (!error && (steps > 1 || ended))
      fakeChecks.set(id, ended && !stopped ? all : fakePart(all, step / steps, stopped));
    const report: JobProgress = { id, phase: "items", done: step, total: null, speed: null, eta: null, message: null };
    fakeProgress.forEach((send) => void send(report));
    if (!ended) return;
    clearInterval(timer);
    const result = error
      ? { state: "failed" as const, message: error }
      : { state: stopped ? ("cancelled" as const) : ("done" as const) };
    fakeFinished.forEach((send) => void send({ id, result }));
  }, 500);
  return id;
}

/** What the check `id` has found so far; all of it once its job ends. Null before it lists anything. */
export async function listedLink(id: number): Promise<Probe | null> {
  if (inTauri) return invoke("listed_link", { id });
  return fakeChecks.get(id) ?? null;
}

/** The thumbnail of a video a probe listed, as a `data:` URL; null when there's none. */
export async function entryThumbnail(url: string): Promise<string | null> {
  if (inTauri) return invoke("entry_thumbnail", { url });
  await new Promise((resolve) => setTimeout(resolve, 200 + Math.random() * 400));
  const hue = [...url].reduce((sum, c) => sum + c.charCodeAt(0) * 7, 0) % 360;
  const [w, h] = url.includes("/shorts/") ? [180, 320] : [320, 180];
  return `data:image/svg+xml,${encodeURIComponent(
    `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${w} ${h}"><defs><linearGradient id="g" x2="1" y2="1"><stop stop-color="hsl(${hue} 45% 32%)"/><stop offset="1" stop-color="hsl(${(hue + 50) % 360} 55% 58%)"/></linearGradient></defs><rect width="${w}" height="${h}" fill="url(#g)"/><circle cx="${w / 2}" cy="${h / 2}" r="${Math.min(w, h) / 6}" fill="white" fill-opacity="0.25"/></svg>`,
  )}`;
}

/** The size a probed video will download with a preset and height, in bytes; `null` when it isn't known (a
 * playlist, a profile, or a site that doesn't say). */
export async function downloadSize(url: string, preset: string, maxHeight: number | null): Promise<number | null> {
  if (inTauri) return invoke("download_size", { url, preset, maxHeight });
  await new Promise((resolve) => setTimeout(resolve, 500));
  if (fakeLinkKind(url) !== "video") return null;
  if (fakePresets.find((p) => p.id === preset)?.audio) return 4_600_000;
  return Math.round(9_000_000 * ((maxHeight ?? 1080) / 240) ** 1.6);
}

/** The output presets, the default first. */
export async function presets(): Promise<PresetInfo[]> {
  return inTauri ? invoke("presets") : fakePresets;
}

/** Queues a download (one per picked section); follow each one's progress by its entry's id. */
export async function startDownload(request: DownloadRequest): Promise<DownloadEntry[]> {
  if (inTauri) return invoke("start_download", { request });
  const links = request.sections.length > 0 ? request.sections : [request.url];
  return links.map((url) => fakeStart({ ...request, url }));
}

function fakeStart(request: DownloadRequest): DownloadEntry {
  const id = ++fakeJobId;
  const linkKind = fakeLinkKind(request.url);
  const kind = linkKind === "channel" || linkKind === "mix" ? "playlist" : linkKind;
  const preset = fakePresets.find((p) => p.id === request.preset)?.name ?? request.preset;
  const entry: DownloadEntry = {
    id,
    source: "download",
    url: request.url,
    title: {
      video: "Me at the zoo",
      playlist: "Pygame Programming Tutorials",
      mix: "Mix - Sleeping With Sirens - Forever/Always",
      channel: "GDULA - Videos",
      profile: "bsky.app",
    }[linkKind],
    preset: kind === "profile" ? "Original files" : preset,
    state: "queued",
    thumbnail: fakeThumbnail,
    file: null,
    files: null,
    skipped: null,
    size: null,
    error: null,
    types: [],
  };
  const saved = {
    video: { file: "Videos/jawed/Me at the zoo [jNQXAC9IVRw].mp4", files: null, skipped: null, size: 30_000_000 },
    playlist: {
      file: "Videos/Tech With Tim/Pygame Programming Tutorials",
      files: (request.limit ?? 40) - 2,
      skipped: 2,
      size: ((request.limit ?? 40) - 2) * 85_000_000,
    },
    profile: {
      file: "Social/bluesky/bsky.app",
      files: request.limit ?? 50,
      skipped: 0,
      size: 61_000_000,
      types: ["JPEG", "MP4", "PNG"],
    },
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

/** The kept downloads, oldest first. */
export async function downloads(): Promise<DownloadEntry[]> {
  return inTauri ? invoke("downloads") : fakeDownloads;
}

/** Removes a finished download from the list (its files stay); returns the list. */
export async function removeDownload(id: number): Promise<DownloadEntry[]> {
  if (inTauri) return invoke("remove_download", { id });
  fakeDownloads = fakeDownloads.filter((d) => d.id !== id);
  return fakeDownloads;
}

/** Removes every finished download from the list (their files stay); returns what's left. */
/** Removes every finished download from the list, or only those from `source` (their files stay); returns what's
 * left. */
export async function clearDownloads(source: DownloadSource | null = null): Promise<DownloadEntry[]> {
  if (inTauri) return invoke("clear_downloads", { source });
  fakeDownloads = fakeDownloads.filter(
    (d) => d.state === "queued" || d.state === "running" || (source != null && d.source !== source),
  );
  return fakeDownloads;
}

/** Adds a row for a fake scrape or hunt, marked done with its files when `onEnd` runs. */
function fakeTrack(id: number, source: DownloadSource, url: string, title: string, preset: string, folder: string) {
  fakeDownloads = [
    ...fakeDownloads,
    { ...fakeOther, id, source, url, title, preset, state: "running", file: null, files: null },
  ];
  return (files: number) => {
    fakeDownloads = fakeDownloads.map((d) => (d.id === id ? { ...d, state: "done", file: folder, files } : d));
  };
}

// ── Grab ──────────────────────────────────
/** Starts scanning a page (and, deeper, the pages it links to) with a headless browser, saving finds as they're found
 * when asked; its progress arrives as job events with the id. */
export async function startScan(request: ScanRequest): Promise<number> {
  if (inTauri) return invoke("start_scan", { request });
  const { url } = request;
  if (flags.has("scanFails")) return Promise.reject("Scanning needs Chrome, Brave or Edge installed.");
  if (!url.startsWith("http")) return Promise.reject("Not a link.");
  const id = ++fakeJobId;
  const all = fakePage(url);
  const page: PageScan = { url, title: null, finds: [], pages: 0, kept: {} };
  // A deeper scan looks at a page more each step.
  const pages = request.depth > 1 ? 4 * request.depth : 1;
  fakeScans.clear();
  fakeScans.set(id, page);
  // Finds arrive over four seconds, as a real scan's do while the page loads and plays.
  const steps = 8;
  let step = 0;
  const timer = setInterval(() => {
    step += 1;
    page.title = all.title;
    page.finds = all.finds.slice(0, Math.ceil((all.finds.length * step) / steps));
    page.pages = Math.max(1, Math.ceil((pages * step) / steps));
    // Saving as found: the kept kinds' finds save, the newest still saving until the scan ends (ADR-0031).
    if (request.save) {
      const kept = page.finds.filter((find) => request.kinds.includes(find.kind));
      page.kept = Object.fromEntries(
        kept.map((find, n) => {
          const small = find.size != null && find.size < request.minKb * 1024;
          const state = small ? "small" : n >= kept.length - 2 && step < steps ? "saving" : "saved";
          return [find.url, { state }];
        }),
      );
    }
    const report: JobProgress = {
      id,
      phase: "items",
      done: page.finds.length,
      total: null,
      speed: null,
      eta: null,
      message: null,
    };
    fakeProgress.forEach((send) => void send(report));
    const stopped = fakeStopped.has(id);
    if (step < steps && !stopped) return;
    clearInterval(timer);
    const result = stopped ? { state: "cancelled" as const } : { state: "done" as const };
    fakeFinished.forEach((send) => void send({ id, result }));
  }, 500);
  return id;
}

/** What a scan has found so far; all of it once its job ends. */
export async function scannedPage(id: number): Promise<PageScan | null> {
  if (inTauri) return invoke("scanned_page", { id });
  const page = fakeScans.get(id);
  return page ? { ...page, finds: [...page.finds] } : null;
}

/** A find's thumbnail as a `data:` URL; `null` for audio and files, or one that can't be read. */
export async function findThumbnail(url: string, kind: FindKind): Promise<string | null> {
  if (inTauri) return invoke("find_thumbnail", { url, kind });
  await new Promise((resolve) => setTimeout(resolve, 200 + Math.random() * 600));
  if (kind === "audio" || kind === "other") return null;
  // Some tall and some wide, as a page's pictures are.
  const shape = [
    [320, 180],
    [180, 320],
    [320, 90],
  ][url.length % 3];
  return fakePicture((url.length * 37) % 360, shape[0], shape[1]);
}

/** Queues the picked finds into the library; follow each by its id. */
export async function grabFinds(request: GrabRequest): Promise<Picked[]> {
  if (inTauri) return invoke("grab_finds", { request });
  const picked = request.finds.map((pick) => {
    const id = ++fakeJobId;
    const entry: DownloadEntry = {
      id,
      source: "grab",
      url: pick.url,
      title: pick.name,
      preset: pick.kind === "stream" ? "MP4 (H.264)" : "Original file",
      state: "queued",
      thumbnail: null,
      file: null,
      files: null,
      skipped: null,
      size: null,
      error: null,
      types: [],
    };
    fakeDownloads = [...fakeDownloads, entry];
    fakeJob(id, 12_000_000, () => {
      const file = `Grabs/example.com/2026-11-10/${pick.name}`;
      const types = pick.kind === "stream" ? [] : [(pick.name.split(".").pop() ?? "").toUpperCase()];
      fakeDownloads = fakeDownloads.map((d) =>
        d.id === id ? { ...d, state: "done", file, size: 12_000_000, types } : d,
      );
    });
    return { url: pick.url, id, error: null };
  });
  // With "Remove duplicates" on and pictures picked, a row that removes one copy once they're saved (ADR-0029).
  if (request.removeDuplicates && request.finds.filter((pick) => pick.kind === "image").length > 1) {
    const id = ++fakeJobId;
    const folder = "Grabs/example.com/2026-11-10";
    const row: DownloadEntry = {
      id,
      source: "grab",
      url: request.pageUrl,
      title: request.title ?? request.pageUrl,
      preset: "Remove duplicates",
      state: "queued",
      thumbnail: null,
      file: null,
      files: null,
      skipped: null,
      size: null,
      error: null,
      types: [],
    };
    fakeDownloads = [...fakeDownloads, row];
    fakeJob(id, 1_000_000, () => {
      fakeDownloads = fakeDownloads.map((d) => (d.id === id ? { ...d, state: "done", file: folder, files: 1 } : d));
    });
  }
  return picked;
}

/** The page the extension handed to Grab, once. */
export async function handedPage(): Promise<PageScan | null> {
  if (inTauri) return invoke("handed_page");
  if (!flags.has("handed") || fakeHandedTaken) return null;
  fakeHandedTaken = true;
  return fakePage("https://example.com/launch");
}

// ── Scrape ────────────────────────────────
/** Starts mirroring a site or saving a page; follow it by the returned id. */
export async function crawlSite(request: CrawlRequest): Promise<CrawlStarted> {
  if (inTauri) return invoke("crawl_site", { request });
  if (!request.url.startsWith("http")) return Promise.reject("Not a link.");
  const id = ++fakeJobId;
  const site = new URL(request.url).host.replace(/^www\./, "");
  const folder = `Sites/${site}`;
  const open = { mirror: `${folder}/index.html`, page: `${folder}/_pages/index.html` }[request.mode];
  const kind = { mirror: "Mirror", page: "Page" }[request.mode];
  const end = fakeTrack(id, "scrape", request.url, site, kind, folder);
  // A mirror lists its media as it goes, a page's worth at a time (ADR-0033).
  const all = fakeMirrorFinds(request.url);
  const mirror: MirrorMedia = { site, url: request.url, finds: [], kept: {}, busy: request.mode === "mirror" };
  if (request.mode === "mirror") fakeMirrors.set(site, mirror);
  const last = request.mode === "page" ? 1 : Math.min(request.pages, 20);
  let pages = 0;
  const timer = setInterval(() => {
    pages += 1;
    const files = pages * 3;
    mirror.finds = all.slice(0, Math.ceil((all.length * pages) / last));
    const message = request.mode === "page" ? null : pages === 1 ? "1 page" : `${pages} pages`;
    fakeProgress.forEach(
      (send) => void send({ id, phase: "items", done: files, total: null, speed: null, eta: null, message }),
    );
    if (pages < last) return;
    clearInterval(timer);
    mirror.busy = false;
    end(files);
    fakeFinished.forEach((send) => void send({ id, result: { state: "done" } }));
  }, 150);
  return { id, site, folder, open };
}

/** A mirror's images and videos, and how saving each went (ADR-0033). */
export async function mirrorMedia(site: string): Promise<MirrorMedia> {
  if (inTauri) return invoke("mirror_media", { site });
  const mirror = fakeMirrors.get(site);
  if (!mirror) return Promise.reject("There's no mirror of this site.");
  return { ...mirror, finds: [...mirror.finds], kept: { ...mirror.kept } };
}

/** Starts saving a mirror's picked images and videos into it; follow it by the returned id. */
export async function saveMirrorMedia(request: MediaRequest): Promise<number> {
  if (inTauri) return invoke("save_mirror_media", { request });
  const mirror = fakeMirrors.get(request.site);
  if (!mirror) return Promise.reject("There's no mirror of this site.");
  if (mirror.busy) return Promise.reject("Wait for the mirror to finish.");
  const picks = mirror.finds.filter((find) => request.urls.includes(find.url));
  if (picks.length === 0) return Promise.reject("Pick something to save.");
  const id = ++fakeJobId;
  const end = fakeTrack(id, "scrape", mirror.url, mirror.site, "Media", `Sites/${mirror.site}`);
  mirror.busy = true;
  for (const pick of picks) mirror.kept[pick.url] = { state: "saving" };
  let done = 0;
  let saved = 0;
  const timer = setInterval(() => {
    const pick = picks[done];
    // Every other clip is over the most a mirror keeps.
    const big = pick.kind === "video" && Number(pick.name.match(/\d+/)?.[0]) % 2 === 0;
    mirror.kept[pick.url] = { state: big ? "big" : "saved" };
    saved += big ? 0 : 1;
    done += 1;
    const total = picks.length;
    fakeProgress.forEach(
      (send) => void send({ id, phase: "items", done, total, speed: null, eta: null, message: null }),
    );
    if (done < picks.length) return;
    clearInterval(timer);
    mirror.busy = false;
    end(saved);
    fakeFinished.forEach((send) => void send({ id, result: { state: "done" } }));
  }, 120);
  return id;
}

// ── Hunt ──────────────────────────────────
/** The source packs, and the user's packs that couldn't be read. */
export async function huntPacks(): Promise<Packs> {
  return inTauri ? invoke("hunt_packs") : fakePacks;
}

/** Saves an edited pack as the user's; returns every pack, read again. */
export async function savePack(pack: Pack): Promise<Packs> {
  if (inTauri) return invoke("save_pack", { pack });
  const name = pack.name.trim();
  if (!name) return Promise.reject("The name must be 1 to 60 characters.");
  if (!pack.web && pack.sites.length === 0) return Promise.reject("It has no sites and doesn't search the web.");
  fakePacks.packs = fakePacks.packs.map((one) =>
    one.id === pack.id ? { ...pack, name: one.builtIn ? one.name : name, builtIn: one.builtIn } : one,
  );
  return { ...fakePacks };
}

/** Saves a new pack of the user's; returns its id, which Rust makes from the name (ADR-0028). */
export async function createPack(pack: Pack): Promise<string> {
  if (inTauri) return invoke("create_pack", { pack });
  const name = pack.name.trim();
  if (!name) return Promise.reject("The name must be 1 to 60 characters.");
  if (!pack.web && pack.sites.length === 0) return Promise.reject("It has no sites and doesn't search the web.");
  const base =
    name
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, "-")
      .replace(/^-|-$/g, "") || "pack";
  let id = base;
  for (let n = 2; fakePacks.packs.some((one) => one.id === id); n++) id = `${base}-${n}`;
  fakePacks.packs = [...fakePacks.packs, { ...pack, id, name, group: null, builtIn: false }];
  return id;
}

/** Deletes the user's own pack `id`; returns every pack, read again. Built-in packs can't be removed. */
export async function removePack(id: string): Promise<Packs> {
  if (inTauri) return invoke("remove_pack", { id });
  if (fakePacks.packs.find((one) => one.id === id)?.builtIn) return Promise.reject("Built-in packs can't be removed.");
  fakePacks.packs = fakePacks.packs.filter((one) => one.id !== id);
  return { ...fakePacks };
}

/** Starts a hunt; follow it by the returned id, and its sources with `huntStatus`. */
export async function startHunt(request: HuntRequest): Promise<HuntStarted> {
  if (inTauri) return invoke("start_hunt", { request });
  const id = ++fakeJobId;
  const folder = `Images/${request.query.trim()}`;
  const track = fakeTrack(id, "hunt", "", request.query.trim(), "Images", folder);
  const read = fakeHunt(request, {
    report: (done, total, message) =>
      fakeProgress.forEach((send) => void send({ id, phase: "items", done, total, speed: null, eta: null, message })),
    stopped: () => fakeStopped.has(id),
    end: (stopped, saved) => {
      track(saved);
      fakeFinished.forEach((send) => void send({ id, result: { state: stopped ? "cancelled" : "done" } }));
    },
  });
  fakeHuntStatus = { id, read };
  return { id, folder };
}

/** The hunt `id` as it is now (its sources, and its duplicates removed); `null` once a newer hunt started. */
export async function huntStatus(id: number): Promise<HuntStatus | null> {
  if (inTauri) return invoke("hunt_status", { id });
  return fakeHuntStatus?.id === id ? fakeHuntStatus.read() : null;
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

/** The folder with the extension for this version, once it's been downloaded; `null` before (ADR-0040). */
export async function extensionFiles(): Promise<string | null> {
  return inTauri ? invoke("extension_files") : fakeExtensionFolder;
}

/** Downloads the extension for this version from its release when it isn't there yet, then opens its folder. */
export async function openExtensionFiles(): Promise<string> {
  if (inTauri) return invoke("open_extension_files");
  await new Promise((done) => setTimeout(done, 800));
  fakeExtensionFolder = "C:/Users/someone/AppData/Roaming/com.nyaridev.scrappy/browser-extension";
  return fakeExtensionFolder;
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
    .filter(
      (item) =>
        (!query.kind || item.kind === query.kind) && (!query.folder || item.path.startsWith(`${query.folder}/`)),
    )
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
  const present = fakeGallery.filter((item) => !fakeTrashed.has(item.path));
  return {
    items: sorted.slice(query.offset, query.offset + query.limit),
    total: sorted.length,
    kinds: [...new Set(present.map((item) => item.kind))].sort(),
    folders: fakeFolders(present),
  };
}

/** Calls `handler` whenever something in the library folder changes; returns a function that stops it. */
export function onLibraryChanged(handler: () => void): () => void {
  if (!inTauri) return () => {};
  const unlisten = listen("library-changed", () => handler());
  return () => void unlisten.then((stop) => stop());
}

/** A library file's thumbnail as a `data:` URL; `null` when it has no picture. */
export async function thumbnail(path: string): Promise<string | null> {
  if (inTauri) return invoke("thumbnail", { path });
  await new Promise((resolve) => setTimeout(resolve, 200 + Math.random() * 400));
  if (/\.mp3$/.test(path)) return null;
  // Some tall and some wide, as a library's pictures are.
  const shape = [
    [320, 180],
    [180, 320],
    [320, 90],
  ][path.length % 3];
  return fakePicture((path.length * 37) % 360, shape[0], shape[1]);
}

/** Where the Gallery's player streams a library video or audio file from: the app's `scrappy:` protocol. */
export function mediaUrl(path: string): string {
  if (inTauri) return convertFileSrc(path, "scrappy");
  // Previews in a browser play public samples.
  return /\.mp3$/.test(path)
    ? "https://www.w3schools.com/html/horse.mp3"
    : "https://www.w3schools.com/html/mov_bbb.mp4";
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
  const found = () => {
    fakeHashed = true;
    fakeFound.set(id, flags.has("empty") ? [] : fakeDuplicates());
  };
  fakeJob(id, fakeHashed ? 1 : 30, found, false, true);
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

/** Deletes a library file, or a downloaded folder with everything in it, for good. */
export async function deleteFromLibrary(path: string): Promise<void> {
  if (inTauri) return invoke("delete_from_library", { path });
  if (!path.includes("/") && !fakeGallery.some((item) => item.path === path)) {
    return Promise.reject("The library's own folders can't be deleted.");
  }
  for (const item of fakeGallery) {
    if (item.path === path || item.path.startsWith(`${path}/`)) fakeTrashed.add(item.path);
  }
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

/** Saves how downloads run (Rust brings it into range) and returns the settings. */
export async function setDownloadLimits(limits: DownloadLimits): Promise<Settings> {
  if (inTauri) return invoke("set_download_limits", { limits });
  const atOnce = Math.min(20, Math.max(1, Math.round(limits.atOnce)));
  fakeSettings = { ...fakeSettings, downloads: { atOnce, speedLimit: Math.max(0, limits.speedLimit) } };
  return fakeSettings;
}

/** Saves which Hunt packs are left out of its Pack list, and returns the settings (ADR-0028). */
export async function setHiddenPacks(ids: string[]): Promise<Settings> {
  if (inTauri) return invoke("set_hidden_packs", { ids });
  fakeSettings = { ...fakeSettings, hiddenPacks: [...new Set(ids)] };
  return fakeSettings;
}

/** Saves the Gallery player's volume and mute, and returns the settings. */
export async function setPlayer(player: Player): Promise<Settings> {
  if (inTauri) return invoke("set_player", { player });
  fakeSettings = { ...fakeSettings, player: { volume: Math.min(1, Math.max(0, player.volume)), muted: player.muted } };
  return fakeSettings;
}

/** Saves the preferences (Rust checks them and brings them into range) and returns the settings (ADR-0034). */
export async function setPreferences(preferences: Preferences): Promise<Settings> {
  if (inTauri) return invoke("set_preferences", { preferences });
  const { media, proxy } = preferences;
  if (!/^[\w.*-]+(,[\w.*-]+)*$/.test(media.subtitleLanguages.replace(/\s/g, ""))) {
    return Promise.reject("Subtitle languages are codes like en or en,pl.");
  }
  if (!media.nameTemplate.trim()) return Promise.reject("The file name can't be empty or longer than 200 characters.");
  if (proxy.trim() && !/^(https?|socks5h?):\/\/[^@\s/]+/.test(proxy.trim())) {
    return Promise.reject("The proxy starts with http://, https://, socks5:// or socks5h://.");
  }
  fakeSettings = { ...fakeSettings, preferences: { ...preferences, proxy: proxy.trim() } };
  return fakeSettings;
}

/** Where a single video goes with the name `template`, shown with a made-up video. */
export async function nameExample(template: string): Promise<string> {
  if (inTauri) return invoke("name_example", { template });
  const fields: Record<string, string> = {
    title: "How rockets land",
    id: "dQw4w9WgXcQ",
    channel: "Space Lab",
    upload_date: "20260412",
    site: "youtube.com",
    user: "spacelab",
    playlist: "",
  };
  const parts = template.split(/[\\/]/).map((part) => part.replace(/\{(\w+)\}/g, (all, name) => fields[name] ?? all));
  return `Videos/${parts.filter((part) => part.trim()).join("/")}.mp4`;
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

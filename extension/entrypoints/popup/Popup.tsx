// Popup: Scrappy's name and "Grab all" (the page and everything found on it, opened in the app's Grab view), then the
// page as the app reads it (ADR-0042): a card with the video and Download when a site reader knows it, else a small
// "Send page" line. Then the site's sign-in line, and the tab's videos and images from a scan of the page and the
// requests the background script saw, with a Videos / Images switch and a smallest-size filter. A click sends one to
// the app; when the app needs the site's sign-in, the row asks before any cookie is read.

import { Button, EmptyState, Icon, NativeSelect, Notice, Segmented, Skeleton, Spinner } from "iro-ui";
import { CheckIcon, ImageIcon, PlayIcon, TargetIcon } from "iro-ui/icons";
import { useCallback, useEffect, useState } from "react";
import { sendToApp } from "../../lib/app";
import { siteCookies } from "../../lib/cookies";
import { formatDuration } from "../../lib/format";
import { type Found, formatOf, merge, sizeGuess } from "../../lib/media";
import { scanPage } from "../../lib/scan";
import type { CookieGrant } from "../../lib/types/CookieGrant";
import type { MediaKind } from "../../lib/types/MediaKind";
import type { Reply } from "../../lib/types/Reply";
import type { RequestsQuery } from "../background";
import { MediaRow, type SendStatus } from "./MediaRow";
import { SignInLine } from "./SignInLine";

// ── Constants ─────────────────────────────
const ALL_SITES = { origins: ["<all_urls>"] };
const KB = 1000;
const MB = 1000 * KB;
/** The smallest-size filter's steps, per kind; 0 is any size. */
const SMALLEST: Record<MediaKind, { value: number; label: string }[]> = {
  video: [
    { value: 0, label: "Any size" },
    { value: MB, label: "1 MB+" },
    { value: 10 * MB, label: "10 MB+" },
    { value: 100 * MB, label: "100 MB+" },
  ],
  image: [
    { value: 0, label: "Any size" },
    { value: 50 * KB, label: "50 KB+" },
    { value: 200 * KB, label: "200 KB+" },
    { value: MB, label: "1 MB+" },
  ],
};

// ── Types ─────────────────────────────────
type Page =
  | { state: "loading" }
  | { state: "blocked" }
  | { state: "needsAccess" }
  | { state: "ready"; url: string; title: string; found: Found[] };

/** What the app made of the page: still reading, a site reader's answer, or nothing it knows. */
type Look =
  | { state: "loading" }
  | { state: "known"; reply: Extract<Reply, { type: "video" }> | null }
  | { state: "page" };

// ── Private helpers ───────────────────────
function statusOf(reply: Reply): SendStatus {
  switch (reply.type) {
    case "queued":
      return { state: "sent" };
    case "needsCookies":
      return { state: "needsSignIn", site: reply.site };
    case "error":
      return { state: "failed", message: reply.message };
    default:
      return { state: "failed", message: "Scrappy didn't queue it." };
  }
}

async function load(): Promise<Page> {
  const [tab] = await browser.tabs.query({ active: true, currentWindow: true });
  if (tab?.id == null || !tab.url || !/^https?:/.test(tab.url)) return { state: "blocked" };
  if (!(await browser.permissions.contains(ALL_SITES))) return { state: "needsAccess" };
  const query: RequestsQuery = { type: "requests", tabId: tab.id };
  const [scanned, requested] = await Promise.all([
    browser.scripting
      .executeScript({ target: { tabId: tab.id }, func: scanPage })
      .then((results) => (results[0]?.result ?? []) as Found[])
      .catch(() => [] as Found[]),
    (browser.runtime.sendMessage(query) as Promise<Found[]>).catch(() => [] as Found[]),
  ]);
  const referer = [{ name: "Referer", value: tab.url }];
  const fromPage = scanned.map((found) => ({ ...found, format: formatOf(found.url), headers: referer }));
  return { state: "ready", url: tab.url, title: tab.title ?? tab.url, found: merge(requested, fromPage) };
}

/** The page as the app reads it. A site that wants a sign-in is still a known one: its Download asks for it. */
async function lookAt(url: string): Promise<Look> {
  const reply = await sendToApp({ type: "look", url }).catch(() => null);
  if (reply?.type === "video") return { state: "known", reply };
  if (reply?.type === "needsCookies") return { state: "known", reply: null };
  return { state: "page" };
}

/** The card's badge and facts for what a site reader found. */
function described(reply: Extract<Reply, { type: "video" }> | null): { badge: string; facts: string[] } {
  if (!reply) return { badge: "Video", facts: [] };
  const facts = [reply.channel, reply.duration ? formatDuration(reply.duration) : null];
  if (reply.count != null) facts.push(`${reply.count} ${reply.files ? "files" : "videos"}`);
  return {
    badge: reply.files ? "Files" : reply.count != null ? "List" : "Video",
    facts: facts.filter(Boolean) as string[],
  };
}

// ── Public API ────────────────────────────
/** The popup. */
export function Popup() {
  const [page, setPage] = useState<Page>({ state: "loading" });
  const [look, setLook] = useState<Look>({ state: "loading" });
  const [kind, setKind] = useState<MediaKind>("video");
  const [smallest, setSmallest] = useState<Record<MediaKind, number>>({ video: 0, image: 0 });
  const [statuses, setStatuses] = useState<Record<string, SendStatus>>({});
  const [grabbed, setGrabbed] = useState<SendStatus | null>(null);

  const reload = useCallback(() => {
    load().then(setPage, () => setPage({ state: "blocked" }));
  }, []);
  useEffect(reload, [reload]);
  const pageUrl = page.state === "ready" ? page.url : null;
  useEffect(() => {
    if (pageUrl) lookAt(pageUrl).then(setLook);
  }, [pageUrl]);

  if (page.state === "loading") return <Spinner className="popup-spinner" label="Scanning the page" />;
  if (page.state === "blocked") return <EmptyState title="This page can't be scanned" />;
  if (page.state === "needsAccess") {
    return (
      <Notice
        tone="info"
        title="Allow access to pages"
        actions={
          <Button variant="primary" size="sm" onClick={() => browser.permissions.request(ALL_SITES).then(reload)}>
            Allow
          </Button>
        }
      >
        Lets Scrappy list a page's videos and images.
      </Notice>
    );
  }

  const videos = page.found.filter((found) => found.kind === "video" && found.url !== page.url);
  const images = page.found.filter((found) => found.kind === "image");
  const listed = kind === "video" ? videos : images;
  const floor = smallest[kind];
  const shown = floor ? listed.filter((found) => (sizeGuess(found)?.bytes ?? 0) >= floor) : listed;
  const known = look.state === "known" ? look.reply : null;
  const pageItem: Found = { url: page.url, kind: "video", format: null, preview: known?.thumbnail ?? undefined };

  const setStatus = (url: string, status: SendStatus | undefined) =>
    setStatuses((all) => {
      const next = { ...all };
      if (status) next[url] = status;
      else delete next[url];
      return next;
    });

  /** Sends `item`; with `share`, reads that one site's cookies first and sends them along. */
  async function send(item: Found, title: string | null, share?: { site: string; remember: boolean }) {
    if (page.state !== "ready") return;
    setStatus(item.url, { state: "sending" });
    const status: SendStatus = await (async () => {
      const cookies: CookieGrant | null = share
        ? { remember: share.remember, cookies: await siteCookies(share.site) }
        : null;
      const reply = await sendToApp({
        type: "send",
        url: item.url,
        kind: item.kind,
        pageUrl: page.url,
        title,
        headers: item.headers ?? [],
        cookies,
        preset: null,
      });
      return statusOf(reply);
    })().catch((error: Error) => ({ state: "failed", message: error.message }) as const);
    setStatus(item.url, status);
  }

  async function grabAll() {
    if (page.state !== "ready") return;
    setGrabbed({ state: "sending" });
    const finds = page.found.map((found) => ({ url: found.url, mime: found.mime ?? null, size: found.size ?? null }));
    const reply = await sendToApp({ type: "grab", pageUrl: page.url, title: page.title, finds }).catch(
      (error: Error) => ({ type: "error", message: error.message }) as const,
    );
    setGrabbed(
      reply.type === "opened"
        ? { state: "sent" }
        : { state: "failed", message: reply.type === "error" ? reply.message : "Scrappy didn't open it." },
    );
  }

  return (
    <>
      <div className="popup-head">
        <img className="popup-logo" src="/icon/48.png" alt="" />
        <span className="popup-name">Scrappy</span>
        <Button
          size="sm"
          variant="ghost"
          icon={<Icon icon={grabbed?.state === "sent" ? CheckIcon : TargetIcon} />}
          loading={grabbed?.state === "sending"}
          disabled={grabbed?.state === "sent"}
          onClick={() => void grabAll()}
        >
          {grabbed?.state === "sent" ? "Opened in Grab" : "Grab all"}
        </Button>
      </div>
      {grabbed?.state === "failed" && <span className="popup-error">{grabbed.message}</span>}
      <ul className="rows rows-page" aria-label="This page" aria-busy={look.state === "loading"}>
        {look.state === "loading" ? (
          <li className="row">
            <Skeleton className="row-preview" />
            <span className="row-text">
              <Skeleton width="80%" />
              <Skeleton width="45%" />
            </span>
          </li>
        ) : (
          <MediaRow
            item={pageItem}
            page
            compact={look.state === "page"}
            title={known?.title ?? page.title}
            {...(look.state === "known" ? described(known) : {})}
            status={statuses[page.url]}
            onSend={() => void send(pageItem, page.title)}
            onShare={(site, remember) => void send(pageItem, page.title, { site, remember })}
            onCancel={() => setStatus(page.url, undefined)}
          />
        )}
      </ul>
      <SignInLine url={page.url} />
      <div className="popup-filters">
        <Segmented
          full
          size="sm"
          aria-label="Show"
          value={kind}
          onValueChange={setKind}
          options={[
            { value: "video", label: `Videos ${videos.length}`, icon: <Icon icon={PlayIcon} /> },
            { value: "image", label: `Images ${images.length}`, icon: <Icon icon={ImageIcon} /> },
          ]}
        />
        <NativeSelect
          size="sm"
          aria-label="Smallest size"
          value={floor}
          onChange={(event) => setSmallest((all) => ({ ...all, [kind]: Number(event.target.value) }))}
        >
          {SMALLEST[kind].map((step) => (
            <option key={step.value} value={step.value}>
              {step.label}
            </option>
          ))}
        </NativeSelect>
      </div>
      <ul className="rows" aria-label={kind === "video" ? "Videos" : "Images"}>
        {shown.map((item) => (
          <MediaRow
            key={item.url}
            item={item}
            status={statuses[item.url]}
            onSend={() => void send(item, item.kind === "image" ? null : page.title)}
            onShare={(site, remember) => void send(item, item.kind === "image" ? null : page.title, { site, remember })}
            onCancel={() => setStatus(item.url, undefined)}
          />
        ))}
      </ul>
      {shown.length === 0 && (
        <EmptyState
          title={listed.length > 0 ? "None this size" : kind === "video" ? "No other videos found" : "No images found"}
        />
      )}
    </>
  );
}

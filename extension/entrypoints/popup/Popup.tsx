// Popup: the current tab's videos and images, from a scan of the page and the requests the background script saw,
// with a Videos / Images switch. A click sends one to the app; when the app needs the site's sign-in, the row asks
// before any cookie is read. "Grab all" opens the page and everything found on it in the app's Grab view.

import { Button, EmptyState, Icon, Notice, Segmented, Spinner } from "iro-ui";
import { ImageIcon, PlayIcon, TargetIcon } from "iro-ui/icons";
import { useCallback, useEffect, useState } from "react";
import { sendToApp } from "../../lib/app";
import { siteCookies } from "../../lib/cookies";
import { type Found, formatOf, merge } from "../../lib/media";
import { scanPage } from "../../lib/scan";
import type { CookieGrant } from "../../lib/types/CookieGrant";
import type { MediaKind } from "../../lib/types/MediaKind";
import type { Reply } from "../../lib/types/Reply";
import type { RequestsQuery } from "../background";
import { MediaRow, type SendStatus } from "./MediaRow";

// ── Constants ─────────────────────────────
const ALL_SITES = { origins: ["<all_urls>"] };

// ── Types ─────────────────────────────────
type Page =
  | { state: "loading" }
  | { state: "blocked" }
  | { state: "needsAccess" }
  | { state: "ready"; url: string; title: string; found: Found[] };

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

// ── Public API ────────────────────────────
/** The popup. */
export function Popup() {
  const [page, setPage] = useState<Page>({ state: "loading" });
  const [kind, setKind] = useState<MediaKind>("video");
  const [statuses, setStatuses] = useState<Record<string, SendStatus>>({});
  const [grabbed, setGrabbed] = useState<SendStatus | null>(null);

  const reload = useCallback(() => {
    load().then(setPage, () => setPage({ state: "blocked" }));
  }, []);
  useEffect(reload, [reload]);

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

  const pageItem: Found = { url: page.url, kind: "video", format: null };
  const videos = page.found.filter((found) => found.kind === "video" && found.url !== page.url);
  const images = page.found.filter((found) => found.kind === "image");
  const shown = kind === "video" ? videos : images;

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
      <div className="popup-grab">
        <Button
          size="sm"
          icon={<Icon icon={TargetIcon} />}
          loading={grabbed?.state === "sending"}
          disabled={grabbed?.state === "sent"}
          onClick={() => void grabAll()}
        >
          {grabbed?.state === "sent" ? "Opened in Scrappy" : "Grab all"}
        </Button>
        {grabbed?.state === "failed" && <span className="popup-grab-error">{grabbed.message}</span>}
      </div>
      <Segmented
        full
        size="sm"
        aria-label="Show"
        value={kind}
        onValueChange={setKind}
        options={[
          { value: "video", label: `Videos ${videos.length + 1}`, icon: <Icon icon={PlayIcon} /> },
          { value: "image", label: `Images ${images.length}`, icon: <Icon icon={ImageIcon} /> },
        ]}
      />
      <ul className="rows" aria-label={kind === "video" ? "Videos" : "Images"}>
        {kind === "video" && (
          <MediaRow
            item={pageItem}
            page
            title={page.title}
            status={statuses[page.url]}
            onSend={() => void send(pageItem, page.title)}
            onShare={(site, remember) => void send(pageItem, page.title, { site, remember })}
            onCancel={() => setStatus(page.url, undefined)}
          />
        )}
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
      {kind === "image" && images.length === 0 && <EmptyState title="No images found" />}
    </>
  );
}

// Background: notes the video and audio each tab loads over the network (HLS and DASH playlists, files loaded by
// scripts), with their size and allowed request headers, and hands a tab's list to the popup. Kept in memory only
// and dropped when the tab navigates or closes (ADR-0006: no storage). It also sends what an in-page button asks for
// to the app, which content scripts can't reach (ADR-0035).

import { sendToApp } from "../lib/app";
import type { DownloadAsk, Outcome } from "../lib/asks";
import { bestStream, type Found, formatOf, isMediaResponse, sizeOf } from "../lib/media";
import type { Header } from "../lib/types/Header";

// ── Constants ─────────────────────────────
const MAX_PER_TAB = 300;
/** The request headers worth passing on; cookies and credentials never leave the extension this way. */
const KEPT_HEADERS = ["referer", "origin", "user-agent"];
// The type names are the same strings in every browser; the typings call them an enum.
const TYPES = ["media", "xmlhttprequest", "other", "object"] as unknown as Browser.webRequest.ResourceType[];

// ── Types ─────────────────────────────────
/** The popup's question. */
export interface RequestsQuery {
  type: "requests";
  tabId: number;
}

// ── Private helpers ───────────────────────
/** Whether a site reader knows `page`, so the page goes rather than a stream it loaded (ADR-0042). */
async function knownPage(page: string): Promise<boolean> {
  const reply = await sendToApp({ type: "look", url: page }).catch(() => null);
  return reply?.type === "video" || reply?.type === "needsCookies";
}

/** Sends a button's link to the app: the link it named, else the page when a site reader knows it, else the tab's
 * best stream, else the page itself. */
async function download(ask: DownloadAsk, tab: Browser.tabs.Tab | undefined, found: Found[]): Promise<Outcome> {
  const page = tab?.url;
  if (!page) return { state: "failed", message: "This page can't be sent." };
  const stream = ask.url || (await knownPage(page)) ? undefined : bestStream(found);
  const url = ask.url ?? stream?.url ?? page;
  const headers = stream?.headers ?? (url === page ? [] : [{ name: "Referer", value: page }]);
  try {
    const reply = await sendToApp({
      type: "send",
      url,
      kind: "video",
      pageUrl: page,
      title: ask.title ?? tab?.title ?? null,
      headers,
      cookies: null,
    });
    if (reply.type === "queued") return { state: "sent" };
    if (reply.type === "needsCookies") return { state: "needsSignIn", site: reply.site };
    return { state: "failed", message: reply.type === "error" ? reply.message : "Scrappy didn't queue it." };
  } catch (error) {
    return { state: "failed", message: (error as Error).message };
  }
}

export default defineBackground(() => {
  const byTab = new Map<number, Map<string, Found>>();
  const headersByRequest = new Map<string, Header[]>();

  browser.webRequest.onSendHeaders.addListener(
    (details) => {
      if (details.tabId < 0) return;
      const kept = (details.requestHeaders ?? [])
        .filter((header) => KEPT_HEADERS.includes(header.name.toLowerCase()) && header.value)
        .map((header) => ({ name: header.name, value: header.value ?? "" }));
      headersByRequest.set(details.requestId, kept);
    },
    { urls: ["<all_urls>"], types: TYPES },
    // Chrome hides Referer and Origin without "extraHeaders"; Firefox doesn't know that option.
    import.meta.env.FIREFOX ? ["requestHeaders"] : ["requestHeaders", "extraHeaders"],
  );

  browser.webRequest.onHeadersReceived.addListener(
    (details) => {
      const headers = headersByRequest.get(details.requestId);
      headersByRequest.delete(details.requestId);
      if (details.tabId < 0 || details.statusCode >= 400) return;
      const header = (name: string) =>
        details.responseHeaders?.find((h) => h.name.toLowerCase() === name)?.value ?? undefined;
      const mime = header("content-type") ?? "";
      if (!isMediaResponse(details.url, mime)) return;
      const found = byTab.get(details.tabId) ?? new Map<string, Found>();
      if (found.size >= MAX_PER_TAB) return;
      found.set(details.url, {
        url: details.url,
        kind: "video",
        format: formatOf(details.url, mime),
        size: sizeOf(header("content-range"), header("content-length")),
        mime: mime || undefined,
        headers,
      });
      byTab.set(details.tabId, found);
    },
    { urls: ["<all_urls>"], types: TYPES },
    ["responseHeaders"],
  );

  browser.webRequest.onErrorOccurred.addListener((details) => headersByRequest.delete(details.requestId), {
    urls: ["<all_urls>"],
  });

  browser.tabs.onUpdated.addListener((tabId, change) => {
    if (change.status === "loading" && change.url) byTab.delete(tabId);
  });
  browser.tabs.onRemoved.addListener((tabId) => byTab.delete(tabId));

  browser.runtime.onMessage.addListener((message: RequestsQuery | DownloadAsk, sender, reply) => {
    if (message?.type === "requests") {
      reply([...(byTab.get(message.tabId)?.values() ?? [])]);
    } else if (message?.type === "download") {
      const found = [...(byTab.get(sender.tab?.id ?? -1)?.values() ?? [])];
      void download(message, sender.tab, found).then(reply);
      // The answer comes later: keeps the channel open for it.
      return true;
    }
  });
});

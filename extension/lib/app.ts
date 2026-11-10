// App: the only way to the Scrappy app, through native messaging. The browser starts scrappy-host, which relays to
// the app and starts it when it's closed (ADR-0006).

import type { Reply } from "./types/Reply";
import type { Request } from "./types/Request";

// ── Constants ─────────────────────────────
const HOST = "com.nyaridev.scrappy";
/** A video send waits for the app to read the link with yt-dlp first. */
const TIMEOUT_MS = 120_000;

// ── Public API ────────────────────────────
/** Sends one request and waits for the app's reply; rejects with a readable message when the app can't be reached. */
export function sendToApp(request: Request): Promise<Reply> {
  return new Promise((resolve, reject) => {
    const port = browser.runtime.connectNative(HOST);
    const timer = setTimeout(() => {
      port.disconnect();
      reject(new Error("Scrappy didn't answer."));
    }, TIMEOUT_MS);
    port.onMessage.addListener((reply: Reply) => {
      clearTimeout(timer);
      port.disconnect();
      resolve(reply);
    });
    port.onDisconnect.addListener(() => {
      clearTimeout(timer);
      // Chrome sets `lastError`; Firefox puts the reason on the port.
      const reason = browser.runtime.lastError?.message ?? (port as { error?: { message?: string } }).error?.message;
      reject(new Error(reason ? `Can't reach Scrappy: ${reason}` : "Can't reach Scrappy."));
    });
    port.postMessage(request);
  });
}

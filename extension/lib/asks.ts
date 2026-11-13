// Asks: what the in-page buttons ask the background script, which alone can reach the app (ADR-0035), and its answer.

/** Download this link, or with `url` null, the video the tab plays: the page when a site reader knows it, else the
 * stream it loaded, else the page. */
export interface DownloadAsk {
  type: "download";
  url: string | null;
  title: string | null;
  /** A preset's id for a button that names its format (MP4, MP3); `null` for the start preset. */
  preset: string | null;
}

/** How a send went. */
export type Outcome = { state: "sent" } | { state: "needsSignIn"; site: string } | { state: "failed"; message: string };

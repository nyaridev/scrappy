// Button: one in-page Download button in a closed shadow root (ADR-0035). A click asks the background script to send
// its link to the app, and the button shows Sending, then Queued, a sign-in to share from the popup, or the error.

import { SEND_TIMEOUT_MS } from "../app";
import type { DownloadAsk, Outcome } from "../asks";
import { type Look, lookCss } from "./looks";

// ── Constants ─────────────────────────────
// Icons as path lists: the DOM is built by hand, since pages like YouTube refuse `innerHTML` (Trusted Types).
/** Scrappy's arrow into a tray; `~` marks a dimmed path. */
const ARROW = ["M12 3.5v11M7.5 10.5 12 15l4.5-4.5", "~M5 16.5v1.5a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2v-1.5"];
const SPINNER = ["M12 3a9 9 0 1 0 9 9"];
const CHECK = ["m5 12.5 4.5 4.5L19 7.5"];
const ALERT = ["M12 8v5", "M12 16.5v.01", "M21 12a9 9 0 1 1-18 0 9 9 0 0 1 18 0"];
const SVG = "http://www.w3.org/2000/svg";
/** How long Queued shows before the button can send again. */
const QUEUED_MS = 4000;
/** The background answers within its own send limit; past this, it was stopped and never will. */
const GIVE_UP_MS = SEND_TIMEOUT_MS + 5000;

// ── Types ─────────────────────────────────
type State = "idle" | "sending" | Outcome["state"];

/** What a button sends: a link, or `null` for the video the tab plays. */
export interface Target {
  url: string | null;
  title: string | null;
}

/** A placed button: its element, and a way to point it at another link when the page changes in place. */
export interface PageButton {
  host: HTMLElement;
  aim: (target: Target) => void;
}

// ── Private helpers ───────────────────────
const LOOK_OF: Record<State, { icon: string[]; label: string; tip: string }> = {
  idle: { icon: ARROW, label: "Download", tip: "Download with Scrappy" },
  sending: { icon: SPINNER, label: "Sending", tip: "Sending to Scrappy" },
  sent: { icon: CHECK, label: "Queued", tip: "Downloading in Scrappy" },
  needsSignIn: { icon: ALERT, label: "Sign-in needed", tip: "" },
  failed: { icon: ALERT, label: "Failed", tip: "" },
};

function icon(paths: string[]): SVGSVGElement {
  const svg = document.createElementNS(SVG, "svg");
  svg.setAttribute("viewBox", "0 0 24 24");
  svg.setAttribute("aria-hidden", "true");
  for (const d of paths) {
    const path = document.createElementNS(SVG, "path");
    path.setAttribute("d", d.replace("~", ""));
    if (d.startsWith("~")) path.setAttribute("class", "dim");
    svg.append(path);
  }
  return svg;
}

async function ask(target: Target): Promise<Outcome> {
  const message: DownloadAsk = { type: "download", ...target };
  let timer = 0;
  const late = new Promise<Outcome>((done) => {
    timer = window.setTimeout(() => done({ state: "failed", message: "Scrappy didn't answer." }), GIVE_UP_MS);
  });
  try {
    return await Promise.race([browser.runtime.sendMessage(message) as Promise<Outcome>, late]);
  } catch {
    // The extension was updated or reloaded: this page's script lost its way to the background.
    return { state: "failed", message: "Reload the page to use Scrappy here." };
  } finally {
    clearTimeout(timer);
  }
}

// ── Public API ────────────────────────────
/** Makes a button with `look`, light or dark, aimed at `target`; `target` is read again at each click. */
export function makeButton(look: Look, dark: boolean, target: Target): PageButton {
  const host = document.createElement("scrappy-download");
  const root = host.attachShadow({ mode: "closed" });
  const style = document.createElement("style");
  style.textContent = lookCss(look, dark);
  const button = document.createElement("button");
  button.type = "button";
  const tip = document.createElement("span");
  tip.className = "tip";
  tip.setAttribute("role", "tooltip");
  root.append(style, button, tip);
  let aimed = target;
  let state: State = "idle";
  let timer = 0;

  function show(next: State, reason = "") {
    state = next;
    const shown = LOOK_OF[next];
    const text = reason || shown.tip;
    button.dataset.state = next;
    const label = document.createElement("span");
    label.className = "label";
    label.textContent = shown.label;
    button.replaceChildren(icon(shown.icon), label);
    button.setAttribute("aria-label", `${shown.label}: ${text}`);
    tip.textContent = text;
  }

  // The page must not see these clicks: a post or a card would open under the button.
  button.addEventListener("click", async (event) => {
    event.preventDefault();
    event.stopPropagation();
    if (state === "sending") return;
    clearTimeout(timer);
    show("sending");
    const outcome = await ask(aimed);
    if (outcome.state === "needsSignIn")
      show("needsSignIn", `Open Scrappy's popup to share your ${outcome.site} sign-in.`);
    else if (outcome.state === "failed") show("failed", outcome.message);
    else show("sent");
    if (outcome.state === "sent") timer = window.setTimeout(() => show("idle"), QUEUED_MS);
  });
  for (const type of ["mousedown", "pointerdown", "pointerup", "mouseup"]) {
    button.addEventListener(type, (event) => event.stopPropagation());
  }

  show("idle");
  return {
    host,
    aim: (next) => {
      if (next.url === aimed.url) return;
      aimed = next;
      clearTimeout(timer);
      show("idle");
    },
  };
}

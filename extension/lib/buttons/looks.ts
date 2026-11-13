// Looks: the CSS of an in-page Download button for each site, shaped like that site's own buttons, in its light or
// dark mode, and of the overlay on any other page's video (ADR-0035). It lives in the button's shadow root, so the page
// can't restyle it.

// ── Types ─────────────────────────────────
/** Which site's buttons it copies; `overlay` floats over a video. */
export type Look = "youtube" | "soundcloud" | "x" | "twitch" | "overlay";

// ── Constants ─────────────────────────────
/** Shared by every look: a reset, the icon, the spinner and the tip that shows a state's reason. */
const BASE = `
:host { all: initial; position: relative; display: inline-flex; vertical-align: middle; }
button {
  all: unset; box-sizing: border-box; display: inline-flex; align-items: center; justify-content: center; gap: 6px;
  cursor: pointer; white-space: nowrap; transition: background-color 0.15s, color 0.15s;
}
button:focus-visible { outline: 2px solid currentColor; outline-offset: 2px; }
svg { flex: none; width: 20px; height: 20px; fill: none; stroke: currentColor; stroke-width: 2;
  stroke-linecap: round; stroke-linejoin: round; }
.dim { opacity: 0.5; }
[data-state="sending"] svg { animation: spin 0.9s linear infinite; }
@keyframes spin { to { transform: rotate(360deg); } }
.tip {
  position: absolute; bottom: calc(100% + 6px); left: 50%; z-index: 1; width: max-content; max-width: 260px;
  padding: 6px 8px; border-radius: 6px; background: rgb(20 20 22); color: rgb(240 240 240);
  font: 500 12px/1.35 system-ui, sans-serif; white-space: normal; transform: translateX(-50%);
  opacity: 0; pointer-events: none; transition: opacity 0.15s;
}
:host(:hover) .tip, button:focus-visible + .tip { opacity: 1; }
`;

const LOOKS: Record<Look, { light: string; dark: string; shape: string }> = {
  youtube: {
    shape: `:host { margin-left: 8px; }
      button { height: 36px; padding: 0 16px 0 12px; border-radius: 18px; font: 500 14px Roboto, Arial, sans-serif;
        background: var(--fill); color: var(--ink); }
      button:hover { background: var(--hover); }`,
    light: ":host { --fill: rgb(0 0 0 / 0.05); --hover: rgb(0 0 0 / 0.1); --ink: rgb(15 15 15); }",
    dark: ":host { --fill: rgb(255 255 255 / 0.1); --hover: rgb(255 255 255 / 0.2); --ink: rgb(241 241 241); }",
  },
  soundcloud: {
    shape: `:host { margin-right: 8px; }
      button { height: 32px; padding: 6px 12px; border-radius: 4px;
        font: 600 14px system-ui, -apple-system, "Segoe UI", Roboto, sans-serif; background: var(--fill); color: var(--ink); }
      button:hover { background: var(--hover); }
      svg { width: 16px; height: 16px; }`,
    light: ":host { --fill: rgb(242 242 242); --hover: rgb(229 229 229); --ink: rgb(18 18 18); }",
    dark: ":host { --fill: rgb(48 48 48); --hover: rgb(64 64 64); --ink: rgb(255 255 255); }",
  },
  x: {
    shape: `button { width: 36px; height: 36px; border-radius: 9999px; color: var(--ink); }
      button:hover { background: rgb(29 155 240 / 0.1); color: rgb(29 155 240); }
      svg { width: 19px; height: 19px; }
      .label { position: absolute; width: 1px; height: 1px; overflow: hidden; clip-path: inset(50%); }`,
    light: ":host { --ink: rgb(83 100 113); }",
    dark: ":host { --ink: rgb(113 118 123); }",
  },
  twitch: {
    shape: `:host { margin-left: 10px; }
      button { height: 32px; padding: 0 12px 0 10px; border-radius: 9000px;
        font: 600 14px Inter, Roobert, "Helvetica Neue", Arial, sans-serif; background: var(--fill); color: var(--ink); }
      button:hover { background: var(--hover); }`,
    light: ":host { --fill: rgb(0 0 0 / 0.05); --hover: rgb(0 0 0 / 0.1); --ink: rgb(14 14 16); }",
    dark: ":host { --fill: rgb(83 83 95 / 0.38); --hover: rgb(83 83 95 / 0.55); --ink: rgb(239 239 241); }",
  },
  overlay: {
    shape: `button { height: 32px; padding: 0 12px 0 9px; border-radius: 16px; background: rgb(15 15 18 / 0.78);
        color: rgb(255 255 255); font: 600 13px system-ui, -apple-system, "Segoe UI", sans-serif;
        box-shadow: 0 2px 10px rgb(0 0 0 / 0.35); backdrop-filter: blur(8px); }
      button:hover { background: rgb(15 15 18 / 0.92); }
      svg { color: rgb(240 135 106); }
      .tip { top: calc(100% + 6px); bottom: auto; left: auto; right: 0; transform: none; }`,
    light: "",
    dark: "",
  },
};

// ── Public API ────────────────────────────
/** The CSS for one look, light or dark. */
export function lookCss(look: Look, dark: boolean): string {
  const { shape, light, dark: darkColors } = LOOKS[look];
  return BASE + shape + (dark ? darkColors : light);
}

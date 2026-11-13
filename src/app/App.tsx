// App: the header (name, which opens Home, the view tabs in the middle, the Downloads and Settings buttons) over the
// selected view, on the backdrop.
// Until there's a library, the first-run screen takes the tabs' place, then the tools step while a tool is missing.
// The header is also the title bar, following the frame Rust gave the window (ADR-0011). A page the extension hands to
// Grab opens the Grab view. The browser's own right-click menu is off, except in text fields and selectable text.
// A view stays mounted once opened, hidden while another shows, so its fields, results and scroll are there on return;
// `ViewShown` counts its showings, so loads that ran on mount run again.

import { Icon, IconButton, Tabs, Tooltip } from "iro-ui";
import { ArrowUpCircleIcon } from "iro-ui/icons";
import { useCallback, useEffect, useState } from "react";
import icon from "../../assets/brand/icon.svg";
import { SetupView } from "../features/setup/SetupView";
import { ToolsStep } from "../features/setup/ToolsStep";
import { appInfo, libraryStatus } from "../lib/backend";
import { AppBackdrop } from "./AppBackdrop";
import { KeptView } from "./KeptView";
import { OpenView, SETTINGS } from "./openView";
import { SettingsProvider } from "./SettingsProvider";
import { useCached } from "./useCached";
import { useHandedPage } from "./useHandedPage";
import { useUpdate } from "./useUpdate";
import { HEADER_VIEWS, MAIN_VIEWS } from "./views";
import { WindowControls } from "./WindowControls";

// ── Constants ─────────────────────────────
const ALL_VIEWS = [...MAIN_VIEWS, ...HEADER_VIEWS];
const TABS = MAIN_VIEWS.map(({ id, label, icon }) => ({ value: id, label, icon: <Icon icon={icon} /> }));
// Where the browser's menu stays: it has copy and paste.
const KEEPS_BROWSER_MENU = "input, textarea, [contenteditable], .selectable";

// ── Private helpers ───────────────────────
function blockBrowserMenu(event: MouseEvent) {
  if (!(event.target instanceof Element && event.target.closest(KEEPS_BROWSER_MENU))) event.preventDefault();
}

// ── Public API ────────────────────────────
/** The window's content. */
export function App() {
  const [current, setCurrent] = useState(MAIN_VIEWS[0].id);
  const update = useUpdate();
  // How many times each opened view has been shown.
  const [shown, setShown] = useState<Record<string, number>>({});
  useEffect(() => setShown((all) => ({ ...all, [current]: (all[current] ?? 0) + 1 })), [current]);
  // Cached for the session, so Settings has them in its first frame.
  const [info] = useCached("appInfo", appInfo);
  const [library, setLibrary] = useCached("library", libraryStatus);
  const [toolsDone, setToolsDone] = useState(false);
  const finishTools = useCallback(() => setToolsDone(true), []);
  const openGrab = useCallback(() => setCurrent("grab"), []);
  useHandedPage(openGrab);
  useEffect(() => {
    document.addEventListener("contextmenu", blockBrowserMenu);
    return () => document.removeEventListener("contextmenu", blockBrowserMenu);
  }, []);
  const frame = info?.frame ?? "native";

  const ready = library?.state === "ready" && toolsDone;
  // Only the header and its name drag the window; the tabs and buttons stay clickable.
  const dragRegion = frame !== "native" || undefined;

  return (
    <SettingsProvider>
      <div className="app">
        <AppBackdrop />
        <header className="header" data-frame={frame} data-tauri-drag-region={dragRegion}>
          {ready ? (
            // The name and icon open Home, as a site's logo does; the room around them still drags the window.
            <div className="brand-slot" data-tauri-drag-region={dragRegion}>
              <Tooltip content="Home">
                <button type="button" className="brand brand-home" onClick={() => setCurrent(MAIN_VIEWS[0].id)}>
                  <img className="brand-icon" src={icon} alt="" />
                  <span className="brand-name">Scrappy</span>
                </button>
              </Tooltip>
            </div>
          ) : (
            <span className="brand" data-tauri-drag-region={dragRegion}>
              <img className="brand-icon" src={icon} alt="" data-tauri-drag-region={dragRegion} />
              Scrappy
            </span>
          )}
          {ready && <Tabs variant="nav" aria-label="Views" tabs={TABS} value={current} onValueChange={setCurrent} />}
          <div className="header-end">
            {ready && update?.latest && (
              <IconButton
                icon={<Icon icon={ArrowUpCircleIcon} />}
                label={`Scrappy ${update.latest} is out`}
                variant="primary"
                onClick={() => setCurrent(SETTINGS)}
              />
            )}
            {ready &&
              HEADER_VIEWS.map(({ id, label, icon }) => (
                <IconButton
                  key={id}
                  icon={<Icon icon={icon} />}
                  label={label}
                  active={current === id}
                  aria-current={current === id ? "page" : undefined}
                  onClick={() => setCurrent(id)}
                />
              ))}
            {frame === "custom" && <WindowControls />}
          </div>
        </header>
        {ready ? (
          <OpenView value={setCurrent}>
            <div className="views">
              {ALL_VIEWS.filter(({ id }) => id === current || shown[id]).map(({ id, label, Page }) => (
                <KeptView key={id} label={label} active={id === current} shown={shown[id] ?? 1} Page={Page} />
              ))}
            </div>
          </OpenView>
        ) : (
          <main className="view" aria-label="Setup">
            {library?.state !== "ready" ? (
              library && <SetupView status={library} onReady={setLibrary} />
            ) : (
              <ToolsStep onDone={finishTools} />
            )}
          </main>
        )}
      </div>
    </SettingsProvider>
  );
}

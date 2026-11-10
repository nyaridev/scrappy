// App: the header (name, the view tabs in the middle, the settings button) over the selected view, on the backdrop.
// Until there's a library, the first-run screen takes the tabs' place, then the tools step while a tool is missing.
// The header is also the title bar, following the frame Rust gave the window (ADR-0011). A page the extension hands to
// Grab opens the Grab view.

import { Icon, IconButton, Tabs } from "iro-ui";
import { useCallback, useState } from "react";
import { SetupView } from "../features/setup/SetupView";
import { ToolsStep } from "../features/setup/ToolsStep";
import { appInfo, libraryStatus } from "../lib/backend";
import { AppBackdrop } from "./AppBackdrop";
import { SettingsProvider } from "./SettingsProvider";
import { useCached } from "./useCached";
import { useHandedPage } from "./useHandedPage";
import { MAIN_VIEWS, SETTINGS_VIEW } from "./views";
import { WindowControls } from "./WindowControls";

// ── Constants ─────────────────────────────
const ALL_VIEWS = [...MAIN_VIEWS, SETTINGS_VIEW];
const TABS = MAIN_VIEWS.map(({ id, label, icon }) => ({ value: id, label, icon: <Icon icon={icon} /> }));

// ── Public API ────────────────────────────
/** The window's content. */
export function App() {
  const [current, setCurrent] = useState(MAIN_VIEWS[0].id);
  // Cached for the session, so Settings has them in its first frame.
  const [info] = useCached("appInfo", appInfo);
  const [library, setLibrary] = useCached("library", libraryStatus);
  const [toolsDone, setToolsDone] = useState(false);
  const finishTools = useCallback(() => setToolsDone(true), []);
  const openGrab = useCallback(() => setCurrent("grab"), []);
  useHandedPage(openGrab);
  const frame = info?.frame ?? "native";

  const { Page, label } = ALL_VIEWS.find((view) => view.id === current) ?? MAIN_VIEWS[0];
  const ready = library?.state === "ready" && toolsDone;
  const inSettings = current === SETTINGS_VIEW.id;
  // Only the header and its name drag the window; the tabs and buttons stay clickable.
  const dragRegion = frame !== "native" || undefined;

  return (
    <SettingsProvider>
      <div className="app">
        <AppBackdrop />
        <header className="header" data-frame={frame} data-tauri-drag-region={dragRegion}>
          <span className="brand" data-tauri-drag-region={dragRegion}>
            Scrappy
          </span>
          {ready && <Tabs variant="nav" aria-label="Views" tabs={TABS} value={current} onValueChange={setCurrent} />}
          <div className="header-end">
            {ready && (
              <IconButton
                icon={<Icon icon={SETTINGS_VIEW.icon} />}
                label={SETTINGS_VIEW.label}
                active={inSettings}
                aria-current={inSettings ? "page" : undefined}
                onClick={() => setCurrent(SETTINGS_VIEW.id)}
              />
            )}
            {frame === "custom" && <WindowControls />}
          </div>
        </header>
        <main className="view" aria-label={ready ? label : "Setup"}>
          {library?.state !== "ready" ? (
            library && <SetupView status={library} onReady={setLibrary} />
          ) : toolsDone ? (
            <Page />
          ) : (
            <ToolsStep onDone={finishTools} />
          )}
        </main>
      </div>
    </SettingsProvider>
  );
}

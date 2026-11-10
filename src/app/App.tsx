// App: the header (name, the view tabs in the middle, the settings button) over the selected view.

import { Icon, IconButton, Tabs } from "iro-ui";
import { useState } from "react";
import { MAIN_VIEWS, SETTINGS_VIEW } from "./views";

// ── Constants ─────────────────────────────
const ALL_VIEWS = [...MAIN_VIEWS, SETTINGS_VIEW];
const TABS = MAIN_VIEWS.map(({ id, label, icon }) => ({ value: id, label, icon: <Icon icon={icon} /> }));

// ── Public API ────────────────────────────
/** The window's content. */
export function App() {
  const [current, setCurrent] = useState(MAIN_VIEWS[0].id);
  const { Page, label } = ALL_VIEWS.find((view) => view.id === current) ?? MAIN_VIEWS[0];
  const inSettings = current === SETTINGS_VIEW.id;

  return (
    <div className="app">
      <header className="header">
        <span className="brand">Scrappy</span>
        <Tabs variant="nav" aria-label="Views" tabs={TABS} value={current} onValueChange={setCurrent} />
        <IconButton
          className="header-end"
          icon={<Icon icon={SETTINGS_VIEW.icon} />}
          label={SETTINGS_VIEW.label}
          active={inSettings}
          aria-current={inSettings ? "page" : undefined}
          onClick={() => setCurrent(SETTINGS_VIEW.id)}
        />
      </header>
      <main className="view" aria-label={label}>
        <Page />
      </main>
    </div>
  );
}

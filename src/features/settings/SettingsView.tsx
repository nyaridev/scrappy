// SettingsView: Settings in tabs (ADR-0034), each section a card: Library (the folder, and changing it: ADR-0039),
// Downloads (how they run, the proxy, what they save), Tools, Browser (remembered sign-ins and the extension), Views
// (what Grab, Scrape and Hunt start with, Hunt's packs folder, removing duplicates) and App (the look, the build, its
// updates, starting with the system and the tray).

import { Icon, type TabItem, Tabs } from "iro-ui";
import { DownloadIcon, FolderIcon, GlobeIcon, InfoIcon, SlidersIcon, WrenchIcon } from "iro-ui/icons";
import { useState } from "react";
import { AboutSettings } from "./AboutSettings";
import { BrowserExtension } from "./BrowserExtension";
import { DownloadSettings } from "./DownloadSettings";
import { LibrarySettings } from "./LibrarySettings";
import { LookSettings } from "./LookSettings";
import { SavingSettings } from "./SavingSettings";
import { SignedInSites } from "./SignedInSites";
import { StartSettings } from "./StartSettings";
import { ToolSettings } from "./ToolSettings";
import "./settings.css";

// ── Types ─────────────────────────────────
type Tab = "library" | "downloads" | "tools" | "browser" | "views" | "app";

// ── Constants ─────────────────────────────
/** Each tab and its sections. */
const TABS: TabItem<Tab>[] = [
  { value: "library", label: "Library", icon: <Icon icon={FolderIcon} />, content: <LibrarySettings /> },
  {
    value: "downloads",
    label: "Downloads",
    icon: <Icon icon={DownloadIcon} />,
    content: (
      <>
        <DownloadSettings />
        <SavingSettings />
      </>
    ),
  },
  { value: "tools", label: "Tools", icon: <Icon icon={WrenchIcon} />, content: <ToolSettings /> },
  {
    value: "browser",
    label: "Browser",
    icon: <Icon icon={GlobeIcon} />,
    content: (
      <>
        <SignedInSites />
        <BrowserExtension />
      </>
    ),
  },
  {
    value: "views",
    label: "Views",
    icon: <Icon icon={SlidersIcon} />,
    content: <StartSettings />,
  },
  {
    value: "app",
    label: "App",
    icon: <Icon icon={InfoIcon} />,
    content: (
      <>
        <LookSettings />
        <AboutSettings />
      </>
    ),
  },
];

// ── Public API ────────────────────────────
/** Settings. */
export function SettingsView() {
  const [tab, setTab] = useState<Tab>("library");
  return (
    <div className="settings">
      <Tabs aria-label="Settings" tabs={TABS} value={tab} onValueChange={setTab} />
    </div>
  );
}

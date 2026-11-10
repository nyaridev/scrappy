// Views: the pages the header switches between. Add a view here and give it a folder in `src/features/`.

import type { IconData } from "iro-ui/icons";
import { DownloadIcon, GlobeIcon, HomeIcon, ImagesIcon, SearchIcon, SettingsIcon, TargetIcon } from "iro-ui/icons";
import type { ComponentType } from "react";
import { DownloadView } from "../features/download/DownloadView";
import { GalleryView } from "../features/gallery/GalleryView";
import { GrabView } from "../features/grab/GrabView";
import { HomeView } from "../features/home/HomeView";
import { HuntView } from "../features/hunt/HuntView";
import { ScrapeView } from "../features/scrape/ScrapeView";
import { SettingsView } from "../features/settings/SettingsView";

// ── Types ─────────────────────────────────
export interface View {
  id: string;
  label: string;
  icon: IconData;
  Page: ComponentType;
}

// ── Public API ────────────────────────────
/** The views in the header's tabs, in order. */
export const MAIN_VIEWS: View[] = [
  { id: "home", label: "Home", icon: HomeIcon, Page: HomeView },
  { id: "download", label: "Download", icon: DownloadIcon, Page: DownloadView },
  { id: "grab", label: "Grab", icon: TargetIcon, Page: GrabView },
  { id: "scrape", label: "Scrape", icon: GlobeIcon, Page: ScrapeView },
  { id: "hunt", label: "Hunt", icon: SearchIcon, Page: HuntView },
  { id: "gallery", label: "Gallery", icon: ImagesIcon, Page: GalleryView },
];

/** Settings, opened from the button at the header's right. */
export const SETTINGS_VIEW: View = { id: "settings", label: "Settings", icon: SettingsIcon, Page: SettingsView };

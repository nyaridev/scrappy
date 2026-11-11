// SettingsProvider: the settings Rust keeps, shared by the backdrop and the Settings view, so a switch applies at once.

import { toast } from "iro-ui";
import { createContext, type ReactNode, useContext, useEffect, useState } from "react";
import { getSettings, setDownloadLimits, setLook } from "../lib/backend";
import type { DownloadLimits } from "../lib/types/DownloadLimits";
import type { Look } from "../lib/types/Look";
import type { Settings } from "../lib/types/Settings";

// ── Types ─────────────────────────────────
interface SettingsState {
  /** `null` until Rust answers. */
  settings: Settings | null;
  saveLook: (look: Look) => void;
  saveDownloads: (limits: DownloadLimits) => void;
}

// ── State ─────────────────────────────────
const SettingsContext = createContext<SettingsState | null>(null);

// ── Public API ────────────────────────────
/** Loads the settings once and shares them with everything inside. */
export function SettingsProvider({ children }: { children: ReactNode }) {
  const [settings, setSettings] = useState<Settings | null>(null);
  useEffect(() => void getSettings().then(setSettings), []);

  const save = (saving: Promise<Settings>) =>
    void saving.then(setSettings, (error) =>
      toast({ tone: "error", title: "Couldn't save the settings", description: String(error) }),
    );
  const saveLook = (look: Look) => save(setLook(look));
  const saveDownloads = (limits: DownloadLimits) => save(setDownloadLimits(limits));

  return <SettingsContext value={{ settings, saveLook, saveDownloads }}>{children}</SettingsContext>;
}

/** The settings and ways to save the look and how downloads run. Only inside `SettingsProvider`. */
export function useSettings(): SettingsState {
  const state = useContext(SettingsContext);
  if (!state) throw new Error("useSettings needs a SettingsProvider above it");
  return state;
}

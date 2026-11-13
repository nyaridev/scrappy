// SettingsProvider: the settings Rust keeps, shared by the backdrop and the Settings view, so a switch applies at once.

import { toast } from "iro-ui";
import { createContext, type ReactNode, useContext, useEffect, useState } from "react";
import { getSettings, setDownloadLimits, setHiddenPacks, setLook, setPlayer, setPreferences } from "../lib/backend";
import type { DownloadLimits } from "../lib/types/DownloadLimits";
import type { Look } from "../lib/types/Look";
import type { Player } from "../lib/types/Player";
import type { Preferences } from "../lib/types/Preferences";
import type { Settings } from "../lib/types/Settings";

// ── Types ─────────────────────────────────
interface SettingsState {
  /** `null` until Rust answers. */
  settings: Settings | null;
  saveLook: (look: Look) => void;
  saveDownloads: (limits: DownloadLimits) => void;
  savePlayer: (player: Player) => void;
  /** Hunt packs left out of its Pack list, by id (ADR-0028). */
  saveHiddenPacks: (ids: string[]) => void;
  /** Resolves with Rust's reason when it refused them, for the field that changed to show (ADR-0034). */
  savePreferences: (preferences: Preferences) => Promise<string | null>;
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
  const savePlayer = (player: Player) => save(setPlayer(player));
  const saveHiddenPacks = (ids: string[]) => save(setHiddenPacks(ids));
  const savePreferences = (preferences: Preferences) =>
    setPreferences(preferences).then(
      (saved) => {
        setSettings(saved);
        return null;
      },
      (error) => String(error),
    );

  return (
    <SettingsContext value={{ settings, saveLook, saveDownloads, savePlayer, saveHiddenPacks, savePreferences }}>
      {children}
    </SettingsContext>
  );
}

/** The settings and ways to save the look, how downloads run, the player's sound, the hidden Hunt packs and the
 * preferences. Only inside `SettingsProvider`. */
export function useSettings(): SettingsState {
  const state = useContext(SettingsContext);
  if (!state) throw new Error("useSettings needs a SettingsProvider above it");
  return state;
}

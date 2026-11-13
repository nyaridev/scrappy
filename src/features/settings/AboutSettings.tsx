// AboutSettings: Scrappy's version and system, its own updates from GitHub (check, then update and restart:
// ADR-0037), whether it starts with the system (ADR-0036), and whether closing the window keeps it in the tray
// (ADR-0038).

import { Button, Icon, Panel, ProgressBar, PropertyList, SectionTitle, Switch, toast } from "iro-ui";
import { ArrowUpCircleIcon, InfoIcon, RefreshIcon } from "iro-ui/icons";
import { useState } from "react";
import { useSettings } from "../../app/SettingsProvider";
import { useCached } from "../../app/useCached";
import { useUpdate } from "../../app/useUpdate";
import { appInfo, checkForUpdate, installUpdate } from "../../lib/backend";
import { formatWhen } from "../../lib/format";
import type { Preferences } from "../../lib/types/Preferences";
import type { UpdateStatus } from "../../lib/types/UpdateStatus";

// ── Private helpers ───────────────────────
/** One line on where updates stand. */
function updateLine(status: UpdateStatus): string {
  if (!status.canUpdate) return "Development builds don't update.";
  if (status.step === "checking") return "Checking for updates";
  if (status.step === "downloading") return `Downloading Scrappy ${status.latest ?? ""}`;
  if (status.step === "installing") return "Installing; Scrappy restarts";
  if (status.error) return status.error;
  if (status.latest) return `Scrappy ${status.latest} is out.`;
  return status.checkedAt ? `Up to date. Checked ${formatWhen(status.checkedAt)}.` : "Not checked yet.";
}

// ── Public API ────────────────────────────
/** The About section of Settings. */
export function AboutSettings() {
  const [info] = useCached("appInfo", appInfo);
  const { settings, savePreferences } = useSettings();
  const status = useUpdate();
  const [installError, setInstallError] = useState<string | null>(null);
  const preferences = settings?.preferences;
  const busy = status?.step != null;
  const startLabel = info?.os === "windows" ? "Start with Windows" : "Open at login";
  const trayLabel = info?.os === "macos" ? "Close to the menu bar" : "Close to tray";
  const toTray = preferences?.closeToTray ?? false;

  /** Saves one switch; a refusal (the system wouldn't change its start entry) shows as a toast. */
  async function save(change: Partial<Preferences>) {
    if (!preferences) return;
    const reason = await savePreferences({ ...preferences, ...change });
    if (reason) toast({ tone: "error", title: reason });
  }

  async function install() {
    setInstallError(null);
    await installUpdate().catch((error) => setInstallError(String(error)));
  }

  return (
    <Panel as="section" className="settings-section">
      <SectionTitle icon={<Icon icon={InfoIcon} />}>About</SectionTitle>
      <PropertyList
        properties={[
          { label: "Version", value: status?.current ?? info?.version, mono: true },
          { label: "System", value: info?.os },
        ]}
      />
      {status && (
        <div className="settings-update">
          <span
            className={status.error || installError ? "settings-update-line selectable" : "settings-update-line"}
            data-tone={status.error || installError ? "danger" : undefined}
          >
            {installError ?? updateLine(status)}
          </span>
          {status.step === "downloading" && <ProgressBar value={status.progress} label="Downloading the update" />}
          {status.canUpdate && (
            <div className="settings-update-actions">
              <Button
                size="sm"
                variant="secondary"
                icon={<Icon icon={RefreshIcon} />}
                loading={status.step === "checking"}
                disabled={busy}
                onClick={() => void checkForUpdate()}
              >
                Check now
              </Button>
              {status.latest && (
                <Button
                  size="sm"
                  variant="primary"
                  icon={<Icon icon={ArrowUpCircleIcon} />}
                  loading={status.step === "downloading" || status.step === "installing"}
                  disabled={busy}
                  onClick={() => void install()}
                >
                  Update and restart
                </Button>
              )}
            </div>
          )}
        </div>
      )}
      <div className="settings-switches">
        <Switch
          label="Check for updates"
          checked={preferences?.checkUpdates ?? false}
          disabled={!preferences}
          onCheckedChange={(checkUpdates) => void save({ checkUpdates })}
        />
        <Switch
          label={startLabel}
          checked={preferences?.startWithSystem ?? false}
          disabled={!preferences}
          onCheckedChange={(startWithSystem) => void save({ startWithSystem })}
        />
        <Switch
          label={trayLabel}
          checked={toTray}
          disabled={!preferences}
          onCheckedChange={(closeToTray) => void save({ closeToTray })}
        />
      </div>
      <p className="settings-hint">
        Checks at start and every 6 hours. {startLabel} opens Scrappy {toTray ? "in the tray" : "minimized"}.
        {toTray && " Quit from the tray icon's menu."}
      </p>
    </Panel>
  );
}

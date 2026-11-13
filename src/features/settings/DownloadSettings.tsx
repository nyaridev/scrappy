// DownloadSettings: how many downloads run at once, each one's speed limit, and the proxy every request goes through
// (ADR-0017, ADR-0034).

import { Field, Icon, NumberField, Panel, SectionTitle } from "iro-ui";
import { DownloadIcon } from "iro-ui/icons";
import { useSettings } from "../../app/SettingsProvider";
import { TextSetting } from "./TextSetting";

// ── Public API ────────────────────────────
/** The Downloads section of Settings. */
export function DownloadSettings() {
  const { settings, saveDownloads, savePreferences } = useSettings();
  const limits = settings?.downloads;
  const preferences = settings?.preferences;

  return (
    <Panel as="section" className="settings-section">
      <SectionTitle icon={<Icon icon={DownloadIcon} />}>Downloads</SectionTitle>
      <div className="settings-fields">
        <Field label="Downloads at once">
          <NumberField
            value={limits?.atOnce ?? null}
            min={1}
            max={20}
            disabled={!limits}
            onValueChange={(atOnce) => limits && atOnce != null && saveDownloads({ ...limits, atOnce })}
          />
        </Field>
        <Field label="Speed limit per download (MB/s)" hint="0 for no limit.">
          <NumberField
            value={limits?.speedLimit ?? null}
            min={0}
            step={0.5}
            largeStep={5}
            format={{ maximumFractionDigits: 1 }}
            disabled={!limits}
            onValueChange={(speedLimit) => limits && saveDownloads({ ...limits, speedLimit: speedLimit ?? 0 })}
          />
        </Field>
      </div>
      {preferences && (
        <TextSetting
          label="Proxy"
          hint="For yt-dlp, gallery-dl and every request Scrappy makes. Empty for none."
          placeholder="socks5://127.0.0.1:9050"
          value={preferences.proxy}
          onSave={(proxy) => savePreferences({ ...preferences, proxy })}
        />
      )}
    </Panel>
  );
}

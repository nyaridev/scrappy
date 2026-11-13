// LookSettings: the liquid background, and the system title bar instead of Scrappy's own (ADR-0011).

import { Icon, Panel, SectionTitle, Switch } from "iro-ui";
import { PaletteIcon } from "iro-ui/icons";
import { useSettings } from "../../app/SettingsProvider";

// ── Public API ────────────────────────────
/** The Look section of Settings. */
export function LookSettings() {
  const { settings, saveLook } = useSettings();
  const look = settings?.look;

  return (
    <Panel as="section" className="settings-section">
      <SectionTitle icon={<Icon icon={PaletteIcon} />}>Look</SectionTitle>
      <Switch
        label="Liquid background"
        checked={look?.liquidBackground ?? false}
        disabled={!look}
        onCheckedChange={(on) => look && saveLook({ ...look, liquidBackground: on })}
      />
      <div className="settings-row">
        <Switch
          label="Use the system title bar"
          checked={look?.nativeFrame ?? false}
          disabled={!look}
          onCheckedChange={(on) => look && saveLook({ ...look, nativeFrame: on })}
        />
        <p className="settings-hint">Applies after a restart.</p>
      </div>
    </Panel>
  );
}

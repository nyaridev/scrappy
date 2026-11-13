// ToolSettings: the external tools with their versions, downloading the missing ones or updating them, and whether
// yt-dlp updates itself at start (ADR-0012, ADR-0034).

import { Button, Icon, Panel, SectionTitle, Switch } from "iro-ui";
import { DownloadIcon, WrenchIcon } from "iro-ui/icons";
import { useSettings } from "../../app/SettingsProvider";
import { useTools } from "../../app/useTools";
import { ToolList } from "../../components/ToolList";

// ── Public API ────────────────────────────
/** The Tools section of Settings. */
export function ToolSettings() {
  const { settings, savePreferences } = useSettings();
  const tools = useTools();
  const preferences = settings?.preferences;

  return (
    <Panel as="section" className="settings-section">
      <SectionTitle
        icon={<Icon icon={WrenchIcon} />}
        actions={
          (tools.updatable || tools.installing) && (
            <Button size="sm" icon={<Icon icon={DownloadIcon} />} loading={tools.installing} onClick={tools.install}>
              {tools.missing ? "Download missing" : "Update"}
            </Button>
          )
        }
      >
        Tools
      </SectionTitle>
      {tools.rows && <ToolList rows={tools.rows} />}
      <Switch
        label="Update yt-dlp when Scrappy starts"
        checked={preferences?.updateYtDlp ?? false}
        disabled={!preferences}
        onCheckedChange={(updateYtDlp) => preferences && void savePreferences({ ...preferences, updateYtDlp })}
      />
    </Panel>
  );
}

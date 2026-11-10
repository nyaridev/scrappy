// SettingsView: placeholder until settings arrive; shows which build is running.

import { EmptyState, Icon } from "iro-ui";
import { SettingsIcon } from "iro-ui/icons";
import { useEffect, useState } from "react";
import { appInfo } from "../../lib/backend";
import type { AppInfo } from "../../lib/types/AppInfo";

/** Settings. */
export function SettingsView() {
  const [info, setInfo] = useState<AppInfo | null>(null);
  useEffect(() => void appInfo().then(setInfo), []);

  return (
    <EmptyState icon={<Icon icon={SettingsIcon} size={32} />} title="Settings">
      Download folder, presets, cookies and the look arrive with their milestones.
      {info && <p>{`Scrappy ${info.version} on ${info.os}`}</p>}
    </EmptyState>
  );
}

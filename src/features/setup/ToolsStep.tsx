// ToolsStep: the first-run step after the library, shown while a tool Scrappy can download is missing. Lists the
// tools with their progress; Skip leaves it for Settings.

import { Button, Icon, Panel } from "iro-ui";
import { DownloadIcon } from "iro-ui/icons";
import { useEffect } from "react";
import { useTools } from "../../app/useTools";
import { ToolList } from "../../components/ToolList";

// ── Public API ────────────────────────────
/** Downloads the missing or outdated tools, then hands over to the app; at once when there's nothing to do. */
export function ToolsStep({ onDone }: { onDone: () => void }) {
  const { rows, needed, updatable, missing, installing, install, cancel } = useTools();
  const failed = rows?.some((row) => row.error) ?? false;
  const nothingToDo = rows !== null && !needed && !installing && !failed;

  useEffect(() => {
    if (nothingToDo) onDone();
  }, [nothingToDo, onDone]);

  if (!rows || nothingToDo) return null;

  return (
    <div className="setup">
      <h1 id="tools-title" className="setup-title">
        Download tools
      </h1>
      <Panel as="section" className="setup-card" aria-labelledby="tools-title">
        <ToolList rows={rows} />
        <div className="setup-actions">
          {installing ? (
            <Button variant="ghost" onClick={cancel}>
              Cancel
            </Button>
          ) : (
            <Button variant="ghost" onClick={onDone}>
              {needed ? "Skip" : "Continue"}
            </Button>
          )}
          {updatable && (
            <Button variant="primary" icon={<Icon icon={DownloadIcon} />} loading={installing} onClick={install}>
              {failed ? "Try again" : missing ? "Download" : "Update"}
            </Button>
          )}
        </div>
      </Panel>
    </div>
  );
}

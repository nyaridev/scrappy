// BrowserExtension: the Settings section that connects the browsers to this app, so the Scrappy extension reaches
// it, and shows which ones are connected; and the extension itself for this version, downloaded from its release
// and opened in the file manager for the user to load unpacked (ADR-0040).

import { Button, Icon, Panel, PropertyList, SectionTitle, toast } from "iro-ui";
import { DownloadIcon, FolderOpenIcon, PuzzleIcon } from "iro-ui/icons";
import { useEffect, useState } from "react";
import { useViewShown } from "../../app/viewShown";
import { connectBrowsers, disconnectBrowsers, extensionFiles, hostStatus, openExtensionFiles } from "../../lib/backend";
import type { HostStatus } from "../../lib/types/HostStatus";

// ── Public API ────────────────────────────
/** The section. */
export function BrowserExtension() {
  const [status, setStatus] = useState<HostStatus | null>(null);
  const [busy, setBusy] = useState(false);
  const [files, setFiles] = useState<string | null>(null);
  const [fetching, setFetching] = useState(false);
  const shown = useViewShown();

  // biome-ignore lint/correctness/useExhaustiveDependencies: `shown` is there to check again when Settings shows
  useEffect(() => {
    hostStatus()
      .then(setStatus)
      .catch(() => {});
    extensionFiles()
      .then(setFiles)
      .catch(() => {});
  }, [shown]);

  /** Downloads the extension when it isn't there yet, then opens its folder. */
  const getFiles = () => {
    setFetching(true);
    openExtensionFiles()
      .then(setFiles)
      .catch((error) => toast({ tone: "error", title: "Couldn't get the extension", description: String(error) }))
      .finally(() => setFetching(false));
  };

  const run = (action: () => Promise<HostStatus>, failed: string) => {
    setBusy(true);
    action()
      .then(setStatus)
      .catch((error) => toast({ tone: "error", title: failed, description: String(error) }))
      .finally(() => setBusy(false));
  };

  const connected = status?.connected ?? [];
  return (
    <Panel as="section" className="settings-section">
      <SectionTitle
        icon={<Icon icon={PuzzleIcon} />}
        actions={
          connected.length > 0 ? (
            <Button
              size="sm"
              loading={busy}
              onClick={() => run(disconnectBrowsers, "Couldn't disconnect the browsers")}
            >
              Disconnect
            </Button>
          ) : (
            <Button
              size="sm"
              variant="primary"
              loading={busy}
              disabled={!status?.available}
              onClick={() => run(connectBrowsers, "Couldn't connect the browsers")}
            >
              Connect browsers
            </Button>
          )
        }
      >
        Browser extension
      </SectionTitle>
      <p className="settings-hint">
        {status && !status.available
          ? "scrappy-host isn't next to the app."
          : "Lets the Scrappy extension in Chrome, Brave, Edge, Chromium and Firefox reach this app."}
      </p>
      <PropertyList
        copyLabel="Copy"
        properties={[
          { label: "Connected", value: connected.length > 0 ? connected.join(", ") : "None" },
          ...(files ? [{ label: "Extension files", value: files, mono: true, copy: files }] : []),
        ]}
      />
      <div className="settings-row">
        <div className="settings-actions">
          <Button
            size="sm"
            icon={<Icon icon={files ? FolderOpenIcon : DownloadIcon} />}
            loading={fetching}
            onClick={getFiles}
          >
            {files ? "Open extension folder" : "Download extension"}
          </Button>
        </div>
        <p className="settings-hint">
          In Chrome, Brave or Edge: open the extensions page, turn on Developer mode, then Load unpacked and pick this
          folder.
        </p>
      </div>
    </Panel>
  );
}

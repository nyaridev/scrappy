// BrowserExtension: the Settings section that connects the browsers to this app, so the Scrappy extension reaches
// it, and shows which ones are connected.

import { Button, Icon, PropertyList, SectionTitle, toast } from "iro-ui";
import { PuzzleIcon } from "iro-ui/icons";
import { useEffect, useState } from "react";
import { connectBrowsers, disconnectBrowsers, hostStatus } from "../../lib/backend";
import type { HostStatus } from "../../lib/types/HostStatus";

// ── Public API ────────────────────────────
/** The section. */
export function BrowserExtension() {
  const [status, setStatus] = useState<HostStatus | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    hostStatus()
      .then(setStatus)
      .catch(() => {});
  }, []);

  const run = (action: () => Promise<HostStatus>, failed: string) => {
    setBusy(true);
    action()
      .then(setStatus)
      .catch((error) => toast({ tone: "error", title: failed, description: String(error) }))
      .finally(() => setBusy(false));
  };

  const connected = status?.connected ?? [];
  return (
    <section className="settings-section">
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
        properties={[{ label: "Connected", value: connected.length > 0 ? connected.join(", ") : "None" }]}
      />
    </section>
  );
}

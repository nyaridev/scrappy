// UpdateBanner: a banner under the header once a newer Scrappy is out (ADR-0037): both versions, the running downloads
// a restart stops, the download's progress or the error, "Update and restart" and "Later", all centered. Later hides it
// until the next start, or until a still newer version is found.

import { Banner, Button, Icon, ProgressBar } from "iro-ui";
import { ArrowUpCircleIcon } from "iro-ui/icons";
import { useState } from "react";
import { installUpdate } from "../lib/backend";
import type { UpdateStatus } from "../lib/types/UpdateStatus";
import { useDownloads } from "./useDownloads";

// ── Private helpers ───────────────────────
/** The banner's words for where the update is. */
function lineOf(status: UpdateStatus, latest: string): string {
  if (status.step === "downloading") {
    const done = status.progress == null ? "" : ` ${Math.round(status.progress * 100)}%`;
    return `Downloading Scrappy ${latest}${done}`;
  }
  if (status.step === "installing") return "Installing; Scrappy restarts";
  return `Scrappy ${latest} is out. You have ${status.current}.`;
}

// ── Public API ────────────────────────────
/** The update banner; nothing until a newer version is out, or after it's hidden. */
export function UpdateBanner({ status }: { status: UpdateStatus | null }) {
  const { entries } = useDownloads();
  const [hidden, setHidden] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const latest = status?.canUpdate ? status.latest : null;
  if (!status || !latest || hidden === latest) return null;

  const busy = status.step === "downloading" || status.step === "installing";
  const running = entries.filter((entry) => entry.state === "running" || entry.state === "queued").length;
  const failed = error ?? status.error;

  async function install() {
    setError(null);
    await installUpdate().catch((reason) => setError(String(reason)));
  }

  return (
    <Banner
      className="update-banner"
      icon={<Icon icon={ArrowUpCircleIcon} />}
      actions={
        <>
          <Button
            size="sm"
            variant="primary"
            loading={busy}
            disabled={status.step != null}
            onClick={() => void install()}
          >
            Update and restart
          </Button>
          {!busy && (
            <Button size="sm" variant="ghost" onClick={() => setHidden(latest)}>
              Later
            </Button>
          )}
        </>
      }
    >
      <span className="update-banner-text">
        <span>{lineOf(status, latest)}</span>
        {running > 0 && !busy && (
          <span className="update-banner-note">
            {running === 1 ? "1 download stops" : `${running} downloads stop`} when Scrappy restarts.
          </span>
        )}
        {failed && (
          <span className="update-banner-note selectable" data-tone="danger">
            {failed}
          </span>
        )}
      </span>
      {status.step === "downloading" && (
        <ProgressBar className="update-banner-progress" value={status.progress} label="Downloading the update" />
      )}
    </Banner>
  );
}

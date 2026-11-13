// FindStatus: how a picked find's download is going, laid over its thumbnail: a bar while it downloads, else a badge
// (Queued, Saved, Stopped, Failed with the reason on hover). A find saved as it was found, or a mirror's picked media,
// shows that instead: Saving, Saved, Too small, Already saved, Too big or Failed (ADR-0031, ADR-0033).

import { Badge, Icon, ProgressBar, Spinner, Tooltip } from "iro-ui";
import { AlertCircleIcon, CheckCircleIcon } from "iro-ui/icons";
import { useDownload } from "../../app/useDownloads";
import type { Find } from "../../lib/types/Find";
import type { Kept } from "../../lib/types/Kept";
import type { Picked } from "../../lib/types/Picked";

// ── Private helpers ───────────────────────
function failed(error: string) {
  return (
    <Tooltip content={error}>
      <Badge className="find-state" tone="danger" icon={<Icon icon={AlertCircleIcon} />}>
        Failed
      </Badge>
    </Tooltip>
  );
}

/** How saving a find as it was found, or a mirror's pick, went. */
function KeptBadge({ kept }: { kept: Kept }) {
  switch (kept.state) {
    case "saving":
      return (
        <Badge className="find-state" icon={<Spinner />}>
          Saving
        </Badge>
      );
    case "saved":
      return (
        <Badge className="find-state" tone="success" icon={<Icon icon={CheckCircleIcon} />}>
          Saved
        </Badge>
      );
    case "small":
      return <Badge className="find-state">Too small</Badge>;
    case "known":
      return <Badge className="find-state">Already saved</Badge>;
    case "big":
      return <Badge className="find-state">Too big</Badge>;
    case "failed":
      return failed(kept.message);
  }
}

// ── Public API ────────────────────────────
/** A picked find's download, or how saving it as found went. It reads its own download, so it alone redraws as that
 * one moves. */
export function FindStatus({ find, pick, kept }: { find: Find; pick?: Picked; kept?: Kept }) {
  const { entry, report } = useDownload(pick?.id);
  if (!pick) return kept ? <KeptBadge kept={kept} /> : null;
  const error = pick.error ?? (entry?.state === "failed" ? (entry.error ?? "Failed") : null);
  if (error) return failed(error);
  if (entry?.state === "done") {
    return (
      <Badge className="find-state" tone="success" icon={<Icon icon={CheckCircleIcon} />}>
        Saved
      </Badge>
    );
  }
  if (entry?.state === "cancelled") return <Badge className="find-state">Stopped</Badge>;
  if (!report || report.phase === "queued") return <Badge className="find-state">Queued</Badge>;
  const fraction = report.total ? report.done / report.total : null;
  return (
    <span className="find-progress">
      <ProgressBar size="sm" label={`${find.name} progress`} value={fraction} />
    </span>
  );
}

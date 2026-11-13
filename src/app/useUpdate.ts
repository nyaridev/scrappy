// useUpdate: what Scrappy's own updater knows (ADR-0037), kept current by its `update-changed` reports. The header
// shows the update banner from it (`UpdateBanner`), and Settings its details.

import { useEffect, useState } from "react";
import { onUpdateChanged, updateStatus } from "../lib/backend";
import type { UpdateStatus } from "../lib/types/UpdateStatus";

// ── Public API ────────────────────────────
/** The updater's status; `null` until it's read. */
export function useUpdate(): UpdateStatus | null {
  const [status, setStatus] = useState<UpdateStatus | null>(null);
  useEffect(() => {
    void updateStatus().then(setStatus);
    return onUpdateChanged(setStatus);
  }, []);
  return status;
}

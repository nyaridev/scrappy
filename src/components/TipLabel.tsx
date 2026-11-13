// TipLabel: a segment's label (its icon and words) that says what it means on hover and focus, with iro-ui's Tooltip.

import { Icon, Tooltip } from "iro-ui";
import type { IconData } from "iro-ui/icons";
import type { ReactNode } from "react";

// ── Public API ────────────────────────────
/** `label` with `icon` before it, and `tip` on hover. */
export function TipLabel({ icon, label, tip }: { icon: IconData; label: ReactNode; tip: ReactNode }) {
  return (
    <Tooltip content={tip}>
      <span className="tip-label">
        <Icon icon={icon} />
        {label}
      </span>
    </Tooltip>
  );
}

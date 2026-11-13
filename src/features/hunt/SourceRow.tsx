// SourceRow: one source of a running or finished hunt: its state (a dot and words, one line; a failure's reason in a
// tooltip), the results it went through, and its images saved of its limit (ADR-0024, ADR-0026).

import { ProgressBar, StatusDot, Tooltip } from "iro-ui";
import type { HuntSource } from "../../lib/types/HuntSource";
import type { SourceState } from "../../lib/types/SourceState";

// ── Constants ─────────────────────────────
const TONE: Record<SourceState, "neutral" | "info" | "success" | "warning" | "danger"> = {
  waiting: "neutral",
  searching: "info",
  reading: "info",
  done: "success",
  failed: "danger",
  stopped: "warning",
};

// ── Private helpers ───────────────────────
function words(source: HuntSource): string {
  switch (source.state) {
    case "waiting":
      return "Waiting";
    case "searching":
      return "Searching";
    case "reading":
      return `Reading ${source.read} of ${source.found} results`;
    case "done":
      return source.found === 0 ? "No results" : "Done";
    case "failed":
      return source.problem ?? "Failed";
    case "stopped":
      return "Stopped";
  }
}

// ── Public API ────────────────────────────
/** The row. */
export function SourceRow({ source }: { source: HuntSource }) {
  const active = source.state === "searching" || source.state === "reading";
  const state = (
    <span className="hunt-source-state" data-state={source.state}>
      {words(source)}
    </span>
  );
  return (
    <li className="hunt-source">
      <StatusDot tone={TONE[source.state]} pulse={active ? "steady" : undefined} aria-hidden />
      <span className="hunt-source-name">{source.name}</span>
      {source.problem ? <Tooltip content={source.problem}>{state}</Tooltip> : state}
      <ProgressBar
        className="hunt-source-bar"
        size="sm"
        label={`${source.name}: images saved`}
        value={source.state === "searching" ? null : source.saved / source.limit}
        tone={source.state === "done" ? "success" : source.state === "failed" ? "danger" : "accent"}
      />
      <span className="hunt-source-count">
        {source.saved} of {source.limit}
      </span>
    </li>
  );
}

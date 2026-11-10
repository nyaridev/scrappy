// ToolList: the managed tools, one row each: name and what it's for, what Rust found or what's happening (a green
// check once installed, amber when older than Scrappy's pin), and progress while downloading. The first-run tools step
// and Settings show it.

import { Badge, Icon, ProgressBar, Tooltip } from "iro-ui";
import { AlertCircleIcon, AlertTriangleIcon, CheckCircleIcon } from "iro-ui/icons";
import type { ToolRow } from "../app/useTools";
import { formatBytes, shortVersion } from "../lib/format";
import type { Tool } from "../lib/types/Tool";

// ── Constants ─────────────────────────────
const NAMES: Record<Tool, string> = { "yt-dlp": "yt-dlp", deno: "Deno", "gallery-dl": "gallery-dl", ffmpeg: "FFmpeg" };

/** What each tool is for, so people know what they're downloading. */
const ABOUT: Record<Tool, string> = {
  "yt-dlp": "Downloads video and audio from YouTube and other sites",
  deno: "Lets yt-dlp read YouTube's player code",
  "gallery-dl": "Downloads images and posts from X, Instagram and more",
  ffmpeg: "Converts and checks video and audio files",
};

// ── Private helpers ───────────────────────
function detail({ status, progress, error }: ToolRow): string {
  if (status.job !== null) {
    if (progress?.phase === "unpack") return "Unpacking";
    if (progress?.total) return `${formatBytes(progress.done)} of ${formatBytes(progress.total)}`;
    return "Downloading";
  }
  if (error) return error;
  if (status.outdated) {
    const found = status.version ? shortVersion(status.version) : "Doesn't run";
    return status.downloadable
      ? `${found}, update to ${status.pinnedVersion}`
      : `${found}, older than ${status.pinnedVersion}`;
  }
  if (status.source !== "missing") return status.version ? shortVersion(status.version) : "Installed";
  return status.downloadable ? "Not installed" : "Not found. Install it and add it to PATH.";
}

/** The row's state for styling: installed rows are green, outdated ones amber, failed ones red. */
function stateOf({ status, error }: ToolRow): "busy" | "error" | "outdated" | "installed" | "missing" {
  if (status.job !== null) return "busy";
  if (error) return "error";
  if (status.outdated) return "outdated";
  return status.source === "missing" ? "missing" : "installed";
}

// ── Public API ────────────────────────────
/** The tools and their state. */
export function ToolList({ rows }: { rows: ToolRow[] }) {
  return (
    <ul className="tool-list">
      {rows.map((row) => {
        const { status, progress } = row;
        const state = stateOf(row);
        return (
          <li key={status.tool} className="tool-row" data-state={state}>
            <span className="tool-name">
              {NAMES[status.tool]}
              {status.source === "system" && <Badge>System</Badge>}
            </span>
            <Tooltip content={status.version} disabled={!status.version}>
              <span className="tool-detail">
                {state === "installed" && <Icon icon={CheckCircleIcon} />}
                {state === "outdated" && <Icon icon={AlertTriangleIcon} />}
                {state === "error" && <Icon icon={AlertCircleIcon} />}
                {detail(row)}
              </span>
            </Tooltip>
            <span className="tool-about">{ABOUT[status.tool]}</span>
            {state === "busy" && (
              <ProgressBar
                className="tool-progress"
                size="sm"
                label={`${NAMES[status.tool]} download`}
                value={progress?.total ? progress.done / progress.total : null}
              />
            )}
          </li>
        );
      })}
    </ul>
  );
}

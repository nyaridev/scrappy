// MediaRow: one video or image: a preview, its name, format, size (known, or guessed with `~`) and quality, and the
// button that sends it to the app, which shows the send's progress and result. The page itself gets a labelled
// Download button, or a small "Send page" when no site reader knows it. When the app needs the site's sign-in, the row
// asks first.

import { Badge, Button, Icon, IconButton, Tooltip } from "iro-ui";
import { CheckIcon, DownloadIcon, GlobeIcon, ImageIcon, PlayIcon, SendIcon } from "iro-ui/icons";
import { formatBytes } from "../../lib/format";
import { type Found, nameOf, sizeGuess } from "../../lib/media";

// ── Types ─────────────────────────────────
/** Where a send is. */
export type SendStatus =
  | { state: "sending" }
  | { state: "sent" }
  | { state: "needsSignIn"; site: string }
  | { state: "failed"; message: string };

// ── Public API ────────────────────────────
/** One row. `page` marks the row for the page itself, named by `title` with the `badge` and `facts` given;
 * `compact` makes it a small line with "Send page". `onShare` sends again with the site's cookies, for this download
 * only or remembered. */
export function MediaRow({
  item,
  page,
  compact,
  title,
  badge,
  facts: given,
  status,
  onSend,
  onShare,
  onCancel,
}: {
  item: Found;
  page?: boolean;
  compact?: boolean;
  title?: string;
  badge?: string;
  facts?: string[];
  status?: SendStatus;
  onSend: () => void;
  onShare: (site: string, remember: boolean) => void;
  onCancel: () => void;
}) {
  const size = sizeGuess(item);
  const facts = given ?? [
    page ? new URL(item.url).host : null,
    item.width && item.height ? `${item.width}×${item.height}` : null,
    size ? `${size.estimated ? "~" : ""}${formatBytes(size.bytes)}` : null,
  ];
  const name = title ?? nameOf(item.url);
  const icon = page ? GlobeIcon : item.kind === "image" ? ImageIcon : PlayIcon;
  const label = page ? (badge ?? "Page") : item.format;
  return (
    <li className="row" data-status={status?.state} data-compact={compact || undefined}>
      {!compact && (
        <span className="row-preview">
          {item.preview ? <img src={item.preview} alt="" loading="lazy" /> : <Icon icon={icon} size={20} />}
        </span>
      )}
      <span className="row-text">
        <Tooltip content={item.url}>
          <span className="row-name">{name}</span>
        </Tooltip>
        <span className="row-facts">
          {label && <Badge tone={item.kind === "image" ? "info" : "accent"}>{label}</Badge>}
          {facts.filter(Boolean).join(" · ")}
        </span>
        {status?.state === "failed" && <span className="row-error">{status.message}</span>}
      </span>
      {status?.state === "sent" ? (
        <span className="row-sent">
          <Icon icon={CheckIcon} label="Sent" />
        </span>
      ) : page ? (
        <Button
          size="sm"
          variant={compact ? "ghost" : "primary"}
          icon={<Icon icon={compact ? SendIcon : DownloadIcon} />}
          loading={status?.state === "sending"}
          disabled={status?.state === "needsSignIn"}
          onClick={onSend}
        >
          {status?.state === "failed" ? "Try again" : compact ? "Send page" : "Download"}
        </Button>
      ) : (
        <IconButton
          icon={<Icon icon={SendIcon} />}
          label={status?.state === "failed" ? "Send again" : "Send to Scrappy"}
          loading={status?.state === "sending"}
          disabled={status?.state === "needsSignIn"}
          onClick={onSend}
        />
      )}
      {status?.state === "needsSignIn" && (
        <fieldset className="row-ask" aria-label={`Share your ${status.site} sign-in`}>
          <span className="row-ask-text">
            Share your {status.site} sign-in with Scrappy? It gets this site's cookies only.
          </span>
          <span className="row-ask-actions">
            <Button size="sm" variant="ghost" onClick={onCancel}>
              Cancel
            </Button>
            <Button size="sm" variant="secondary" onClick={() => onShare(status.site, false)}>
              Use once
            </Button>
            <Button size="sm" variant="primary" onClick={() => onShare(status.site, true)}>
              Remember
            </Button>
          </span>
        </fieldset>
      )}
    </li>
  );
}

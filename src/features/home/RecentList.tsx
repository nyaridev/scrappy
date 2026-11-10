// RecentList: the latest downloads, newest first, each with a button that shows it in the file manager.

import { Icon, ListItem, Tooltip } from "iro-ui";
import { ShowInFolderButton } from "../../components/ShowInFolderButton";
import { formatBytes, formatWhen } from "../../lib/format";
import { kindOf } from "../../lib/kinds";
import type { RecentDownload } from "../../lib/types/RecentDownload";

// ── Public API ────────────────────────────
/** The recent downloads. */
export function RecentList({ downloads }: { downloads: RecentDownload[] }) {
  return (
    <ul className="recent" aria-label="Recent downloads">
      {downloads.map((download) => {
        const kind = kindOf(download.kind);
        return (
          <ListItem
            as="li"
            key={download.path}
            start={<Icon icon={kind.icon} label={kind.label} />}
            title={
              <Tooltip content={download.path}>
                <span>{download.name}</span>
              </Tooltip>
            }
            description={[download.site, formatBytes(download.bytes)].filter(Boolean).join(" · ")}
            meta={formatWhen(download.downloadedAt)}
            actions={<ShowInFolderButton path={download.path} />}
          />
        );
      })}
    </ul>
  );
}

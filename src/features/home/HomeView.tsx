// HomeView: the library's numbers (files, size, the last 30 days, per day and per site) and the latest downloads.
// Opens from the session cache and refreshes when a download finishes.

import { BarChart, EmptyState, Icon, Panel, SectionTitle, Stat } from "iro-ui";
import { CalendarIcon, ClockIcon, FileIcon, FolderIcon, GlobeIcon, HomeIcon } from "iro-ui/icons";
import { useEffect } from "react";
import { useCached } from "../../app/useCached";
import { libraryStats, onJobFinished, recentDownloads } from "../../lib/backend";
import { formatBytes, formatDay } from "../../lib/format";
import { kindOf } from "../../lib/kinds";
import { RecentList } from "./RecentList";

// ── Constants ─────────────────────────────
const RECENT = 8;
const TOP_SITES = 5;

// ── Private helpers ───────────────────────
const loadRecent = () => recentDownloads(RECENT);

// ── Public API ────────────────────────────
/** Home */
export function HomeView() {
  const [stats, setStats] = useCached("libraryStats", libraryStats);
  const [recent, setRecent] = useCached("recentDownloads", loadRecent);

  useEffect(
    () =>
      onJobFinished(() => {
        void libraryStats().then(setStats);
        void loadRecent().then(setRecent);
      }),
    [setStats, setRecent],
  );

  if (!stats || !recent) return null;
  if (stats.files === 0) return <EmptyState icon={<Icon icon={HomeIcon} size={32} />} title="No downloads yet" />;

  const month = stats.days.reduce((sum, day) => ({ files: sum.files + day.files, bytes: sum.bytes + day.bytes }), {
    files: 0,
    bytes: 0,
  });
  const sites = stats.sites.slice(0, TOP_SITES);

  return (
    <div className="home">
      <div className="home-stats">
        <Stat
          icon={<Icon icon={FileIcon} />}
          label="Files"
          value={stats.files.toLocaleString()}
          hint={stats.kinds.map((kind) => `${kindOf(kind.name).label} ${kind.files.toLocaleString()}`).join(" · ")}
        />
        <Stat icon={<Icon icon={FolderIcon} />} label="Size" value={formatBytes(stats.bytes)} />
        <Stat
          icon={<Icon icon={CalendarIcon} />}
          label="Last 30 days"
          value={month.files.toLocaleString()}
          hint={formatBytes(month.bytes)}
        />
      </div>

      <Panel as="section" className="home-section" aria-labelledby="home-days">
        <SectionTitle id="home-days" icon={<Icon icon={CalendarIcon} />}>
          Per day
        </SectionTitle>
        <BarChart
          label="Downloads per day, last 30 days"
          labels={stats.days.map((day) => formatDay(day.day))}
          series={[{ name: "Downloads", values: stats.days.map((day) => day.files) }]}
          height={160}
        />
      </Panel>

      <div className="home-columns">
        <Panel as="section" className="home-section" aria-labelledby="home-recent">
          <SectionTitle id="home-recent" icon={<Icon icon={ClockIcon} />}>
            Recent
          </SectionTitle>
          <RecentList downloads={recent} />
        </Panel>

        {sites.length > 0 && (
          <Panel as="section" className="home-section" aria-labelledby="home-sites">
            <SectionTitle id="home-sites" icon={<Icon icon={GlobeIcon} />}>
              Sites
            </SectionTitle>
            <BarChart
              label="Downloads per site"
              labels={sites.map((site) => site.name)}
              series={[{ name: "Downloads", values: sites.map((site) => site.files) }]}
              horizontal
            />
          </Panel>
        )}
      </div>
    </div>
  );
}

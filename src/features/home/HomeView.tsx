// HomeView: the library at a glance: its totals over the last 30 days per day, the latest downloads as tiles (with a
// way to the Gallery), and what kinds and sites it holds. Opens from the session cache and refreshes when a download
// finishes.

import { BarChart, Button, DonutChart, EmptyState, Icon, Panel, SectionTitle, Stat } from "iro-ui";
import { CalendarIcon, ClockIcon, FileIcon, FolderIcon, GlobeIcon, HomeIcon, LayersIcon, SunIcon } from "iro-ui/icons";
import { useEffect } from "react";
import { useOpenView } from "../../app/openView";
import { useCached } from "../../app/useCached";
import { libraryStats, onJobFinished, recentDownloads } from "../../lib/backend";
import { coalesce } from "../../lib/coalesce";
import { formatBytes, formatCount, formatDay } from "../../lib/format";
import { kindOf } from "../../lib/kinds";
import type { Day } from "../../lib/types/Day";
import { RecentList } from "./RecentList";
import "./home.css";

// ── Constants ─────────────────────────────
const RECENT = 8;
const TOP_SITES = 6;
const WEEK = 7;
const RELOAD_MS = 1000;
const GALLERY = "gallery";

// ── Private helpers ───────────────────────
const loadRecent = () => recentDownloads(RECENT);

/** The files and bytes of some days together. */
function total(days: Day[]): { files: number; bytes: number } {
  return days.reduce((sum, day) => ({ files: sum.files + day.files, bytes: sum.bytes + day.bytes }), {
    files: 0,
    bytes: 0,
  });
}

// ── Public API ────────────────────────────
/** Home */
export function HomeView() {
  const [stats, setStats] = useCached("libraryStats", libraryStats);
  const [recent, setRecent] = useCached("recentDownloads", loadRecent);
  const openView = useOpenView();

  // A grab of a thousand files ends a thousand jobs: the numbers are read again at most once a second.
  useEffect(() => {
    const reload = coalesce(() => {
      void libraryStats().then(setStats);
      void loadRecent().then(setRecent);
    }, RELOAD_MS);
    return onJobFinished(reload);
  }, [setStats, setRecent]);

  if (!stats || !recent) return null;
  if (stats.files === 0) return <EmptyState icon={<Icon icon={HomeIcon} size={32} />} title="No downloads yet" />;

  const month = total(stats.days);
  const week = total(stats.days.slice(-WEEK));
  const today = total(stats.days.slice(-1));
  const sites = stats.sites.slice(0, TOP_SITES);

  return (
    <div className="home">
      <Panel as="section" className="home-card" aria-label="Library">
        <div className="home-stats">
          <div className="home-pair">
            <Stat variant="plain" icon={<Icon icon={FileIcon} />} label="Files" value={stats.files.toLocaleString()} />
            <Stat variant="plain" icon={<Icon icon={FolderIcon} />} label="Size" value={formatBytes(stats.bytes)} />
          </div>
          <div className="home-pair">
            <Stat
              variant="plain"
              icon={<Icon icon={SunIcon} />}
              label="Today"
              value={today.files.toLocaleString()}
              hint={formatBytes(today.bytes)}
            />
            <Stat
              variant="plain"
              icon={<Icon icon={CalendarIcon} />}
              label="Last 7 days"
              value={week.files.toLocaleString()}
              hint={formatBytes(week.bytes)}
            />
          </div>
        </div>
        <div className="home-days">
          <div className="home-head">
            <SectionTitle id="home-days" icon={<Icon icon={CalendarIcon} />}>
              Last 30 days
            </SectionTitle>
            <span className="home-facts">
              {formatCount(month.files, "file")} · {formatBytes(month.bytes)}
            </span>
          </div>
          <BarChart
            label="Downloads per day, last 30 days"
            labels={stats.days.map((day) => formatDay(day.day))}
            series={[{ name: "Downloads", values: stats.days.map((day) => day.files) }]}
            height={150}
          />
        </div>
      </Panel>

      {recent.length > 0 && (
        <section className="home-recent" aria-labelledby="home-recent">
          <div className="home-head">
            <SectionTitle id="home-recent" icon={<Icon icon={ClockIcon} />}>
              Recent
            </SectionTitle>
            <Button size="sm" variant="ghost" onClick={() => openView(GALLERY)}>
              Gallery
            </Button>
          </div>
          <RecentList downloads={recent} />
        </section>
      )}

      <div className="home-columns">
        <Panel as="section" className="home-section" aria-labelledby="home-kinds">
          <SectionTitle id="home-kinds" icon={<Icon icon={LayersIcon} />}>
            Types
          </SectionTitle>
          <DonutChart
            label="Library size by type"
            parts={stats.kinds.map((kind) => ({ name: kindOf(kind.name).label, value: kind.bytes }))}
            centerValue={formatBytes(stats.bytes)}
            centerLabel="Size"
            size={140}
            format={formatBytes}
          />
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

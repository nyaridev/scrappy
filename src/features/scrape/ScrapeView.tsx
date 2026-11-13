// ScrapeView: save a whole site to read offline, or one page with everything it needs (ADR-0008); a site's files are
// Grab's (ADR-0031). The mode as tabs on top, the site, a mirror's limits (depth, pages, delay in seconds), start and
// stop, live counts, the folder it went to, opening the result, and a mirror's images and videos to pick from as it
// crawls (`MirrorPicks`, ADR-0033).

import { Button, Field, Icon, IconButton, NumberField, Panel, ProgressBar, Tabs, TextField, toast } from "iro-ui";
import { ExternalLinkIcon, GlobeIcon, LinkIcon, SaveIcon } from "iro-ui/icons";
import { useState } from "react";
import { useSettings } from "../../app/SettingsProvider";
import { ShowInFolderButton } from "../../components/ShowInFolderButton";
import { StopButton } from "../../components/StopButton";
import { openFile } from "../../lib/backend";
import { formatCount } from "../../lib/format";
import type { CrawlMode } from "../../lib/types/CrawlMode";
import { MirrorPicks } from "./MirrorPicks";
import { useCrawl } from "./useCrawl";

// ── Constants ─────────────────────────────
const MODES = [
  { value: "mirror", label: "Mirror site", icon: GlobeIcon },
  { value: "page", label: "Save page", icon: SaveIcon },
] as const;

/** Per mode: the start button and the running state. */
const WORDS: Record<CrawlMode, { start: string; running: string }> = {
  mirror: { start: "Mirror", running: "Mirroring" },
  page: { start: "Save", running: "Saving" },
};

// ── Types ─────────────────────────────────
interface Limits {
  depth: number;
  pages: number;
  delayMs: number;
}

// ── Public API ────────────────────────────
/** Scrape */
export function ScrapeView() {
  const { crawl, start, stop } = useCrawl();
  const [link, setLink] = useState("");
  const [mode, setMode] = useState<CrawlMode>("mirror");
  const starts = useSettings().settings?.preferences?.starts.scrape;
  const [limits, setLimits] = useState<Limits>(() => ({
    depth: starts?.depth ?? 2,
    pages: starts?.pages ?? 100,
    delayMs: starts?.delayMs ?? 500,
  }));
  const running = crawl.state === "running";
  const ready = link.trim() !== "";
  const progress = crawl.state === "failed" || crawl.state === "idle" ? null : crawl.progress;

  const limit = (key: keyof Limits, label: string, min: number, max: number, step = 1) => (
    <Field label={label}>
      <NumberField
        value={limits[key]}
        min={min}
        max={max}
        step={step}
        disabled={running}
        onValueChange={(value) => setLimits((all) => ({ ...all, [key]: value ?? min }))}
      />
    </Field>
  );

  const run = () => {
    if (!ready || running) return;
    void start({ url: link.trim(), mode, ...limits });
  };

  const open = (path: string) =>
    openFile(path).catch((error) =>
      toast({ tone: "error", title: "Couldn't open the page", description: String(error) }),
    );

  const counts = () => {
    if (crawl.state === "idle" || crawl.state === "failed") return "";
    const state = crawl.state === "done" ? "Done" : crawl.state === "stopped" ? "Stopped" : WORDS[crawl.mode].running;
    if (crawl.mode === "page") return state;
    if (!progress) return `${state} · starting`;
    return `${state} · ${progress.message ?? "0 pages"} · ${formatCount(progress.done, "file")}`;
  };

  return (
    <div className="scrape">
      <Panel as="section" className="scrape-card" aria-label="Crawl a site">
        <Tabs
          variant="pill"
          className="fill-tabs"
          aria-label="Mode"
          tabs={MODES.map(({ value, label, icon }) => ({
            value,
            label,
            icon: <Icon icon={icon} />,
            // The mode can't change under a running crawl.
            disabled: running && value !== mode,
          }))}
          value={mode}
          onValueChange={setMode}
        />
        <Field label="Site" error={crawl.state === "failed" ? crawl.message : undefined}>
          <TextField
            value={link}
            onChange={(event) => setLink(event.target.value)}
            onKeyDown={(event) => event.key === "Enter" && run()}
            placeholder="https://"
            spellCheck={false}
            autoComplete="off"
            disabled={running}
            start={<Icon icon={LinkIcon} />}
          />
        </Field>
        {mode === "mirror" && (
          <div className="scrape-limits">
            {limit("depth", "Depth", 1, 10)}
            {limit("pages", "Pages", 1, 10_000, 10)}
            <Field label="Delay (s)">
              <NumberField
                value={limits.delayMs / 1000}
                min={0}
                max={60}
                step={0.1}
                format={{ maximumFractionDigits: 1 }}
                disabled={running}
                onValueChange={(value) => setLimits((all) => ({ ...all, delayMs: Math.round((value ?? 0) * 1000) }))}
              />
            </Field>
          </div>
        )}
        <div className="scrape-actions">
          {running ? (
            <StopButton onStop={stop} />
          ) : (
            <Button
              variant="primary"
              icon={<Icon icon={mode === "page" ? SaveIcon : GlobeIcon} />}
              disabled={!ready}
              onClick={run}
            >
              {WORDS[mode].start}
            </Button>
          )}
        </div>
      </Panel>

      {crawl.state !== "idle" && crawl.state !== "failed" && (
        <Panel as="section" className="scrape-status" aria-label="Crawl">
          {running && <ProgressBar label={crawl.mode === "page" ? "Saving" : "Mirroring"} value={null} />}
          <div className="scrape-counts">
            <span>{counts()}</span>
            <span className="scrape-folder selectable">{crawl.open ?? crawl.folder}</span>
            <ShowInFolderButton path={crawl.open ?? crawl.folder} />
            {crawl.state === "done" && crawl.open && (
              <IconButton
                size="sm"
                icon={<Icon icon={ExternalLinkIcon} />}
                label="Open in the browser"
                onClick={() => crawl.open && void open(crawl.open)}
              />
            )}
          </div>
        </Panel>
      )}

      {crawl.state !== "idle" && crawl.state !== "failed" && crawl.mode === "mirror" && (
        <MirrorPicks site={crawl.site} crawling={running} />
      )}
    </div>
  );
}

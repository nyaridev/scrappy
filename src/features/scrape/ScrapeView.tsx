// ScrapeView: crawl a site for its files, mirror it, or save one page: the link, what to keep, the limits (depth,
// pages, files, delay, smallest size), start and stop, live counts, the folder it went to, and opening the result.

import {
  Button,
  Field,
  Icon,
  IconButton,
  MultiSegmented,
  NumberField,
  Panel,
  ProgressBar,
  Segmented,
  TextField,
  toast,
} from "iro-ui";
import {
  AudioBarsIcon,
  ExternalLinkIcon,
  FileIcon,
  GlobeIcon,
  ImageIcon,
  LinkIcon,
  PlayIcon,
  SaveIcon,
  StopIcon,
} from "iro-ui/icons";
import { type ReactNode, useState } from "react";
import { ShowInFolderButton } from "../../components/ShowInFolderButton";
import { openFile } from "../../lib/backend";
import { formatCount } from "../../lib/format";
import type { CrawlMode } from "../../lib/types/CrawlMode";
import type { FindKind } from "../../lib/types/FindKind";
import { useCrawl } from "./useCrawl";

// ── Constants ─────────────────────────────
const KINDS: { value: FindKind; label: string; icon: ReactNode }[] = [
  { value: "image", label: "Images", icon: <Icon icon={ImageIcon} /> },
  { value: "video", label: "Videos", icon: <Icon icon={PlayIcon} /> },
  { value: "audio", label: "Audio", icon: <Icon icon={AudioBarsIcon} /> },
  { value: "other", label: "Files", icon: <Icon icon={FileIcon} /> },
];

const MODES: { value: CrawlMode; label: string }[] = [
  { value: "files", label: "Files" },
  { value: "mirror", label: "Mirror site" },
  { value: "page", label: "Save page" },
];

/** Per mode: the start button and the running state. */
const WORDS: Record<CrawlMode, { start: string; running: string }> = {
  files: { start: "Crawl", running: "Crawling" },
  mirror: { start: "Mirror", running: "Mirroring" },
  page: { start: "Save", running: "Saving" },
};

// ── Types ─────────────────────────────────
interface Limits {
  depth: number;
  pages: number;
  files: number;
  delayMs: number;
  minKb: number;
}

// ── Public API ────────────────────────────
/** Scrape */
export function ScrapeView() {
  const { crawl, start, stop } = useCrawl();
  const [link, setLink] = useState("");
  const [mode, setMode] = useState<CrawlMode>("files");
  const [kinds, setKinds] = useState<FindKind[]>(["image"]);
  const [limits, setLimits] = useState<Limits>({ depth: 2, pages: 100, files: 200, delayMs: 500, minKb: 10 });
  const running = crawl.state === "running";
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
    if (!link.trim() || running) return;
    void start({ url: link.trim(), mode, kinds, ...limits });
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
    return `${state} · ${progress.message ?? "0 pages"} · ${formatCount(progress.done, "file")} saved`;
  };

  return (
    <div className="scrape">
      <Panel as="section" className="scrape-card" aria-label="Crawl a site">
        <Segmented
          size="sm"
          aria-label="Mode"
          options={MODES}
          value={mode}
          onValueChange={setMode}
          disabled={running}
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
        {mode === "files" && (
          <Field label="Keep">
            <MultiSegmented
              size="sm"
              aria-label="Kinds of files"
              options={KINDS}
              value={kinds}
              disabled={running}
              onValueChange={(next) => next.length > 0 && setKinds(next)}
            />
          </Field>
        )}
        {mode !== "page" && (
          <div className="scrape-limits">
            {limit("depth", "Depth", 1, 10)}
            {limit("pages", "Pages", 1, 10_000, 10)}
            {limit("files", "Files", 1, 10_000, 10)}
            {limit("delayMs", "Delay (ms)", 0, 60_000, 100)}
            {mode === "files" && limit("minKb", "Smallest (KB)", 0, 100_000, 5)}
          </div>
        )}
        <div className="scrape-actions">
          {running ? (
            <Button icon={<Icon icon={StopIcon} />} onClick={stop}>
              Stop
            </Button>
          ) : (
            <Button
              variant="primary"
              icon={<Icon icon={mode === "page" ? SaveIcon : GlobeIcon} />}
              disabled={!link.trim()}
              onClick={run}
            >
              {WORDS[mode].start}
            </Button>
          )}
        </div>
      </Panel>

      {crawl.state !== "idle" && crawl.state !== "failed" && (
        <Panel as="section" className="scrape-status" aria-label="Crawl">
          {running && (
            <ProgressBar
              label={crawl.mode === "page" ? "Saving" : "Files saved"}
              value={crawl.mode !== "page" && progress?.total ? progress.done / progress.total : null}
            />
          )}
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
    </div>
  );
}

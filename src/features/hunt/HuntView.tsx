// HuntView: images on a topic from across the web: the topic, the pack and which of its places to search, a switch
// per kind of image data, the limits, start and stop, live counts, the folder, and a duplicate scan of it when done.

import {
  Button,
  Checkbox,
  Field,
  Icon,
  Notice,
  NumberField,
  Panel,
  ProgressBar,
  Select,
  Switch,
  TextField,
} from "iro-ui";
import { DuplicateIcon, SearchIcon, StopIcon } from "iro-ui/icons";
import { useEffect, useState } from "react";
import { useViewShown } from "../../app/viewShown";
import { DuplicatesPanel } from "../../components/DuplicatesPanel";
import { ShowInFolderButton } from "../../components/ShowInFolderButton";
import { huntPacks } from "../../lib/backend";
import { formatCount } from "../../lib/format";
import type { ImageKind } from "../../lib/types/ImageKind";
import type { Packs } from "../../lib/types/Packs";
import { useHunt } from "./useHunt";

// ── Constants ─────────────────────────────
const KINDS: { kind: ImageKind; label: string }[] = [
  { kind: "img", label: "Images" },
  { kind: "responsive", label: "Lazy and high-res" },
  { kind: "linked", label: "Full-size links" },
  { kind: "background", label: "Backgrounds" },
  { kind: "preview", label: "Preview image" },
  { kind: "inline", label: "Inline SVG and data" },
];
const FIRST_KINDS: ImageKind[] = ["img", "responsive", "linked", "background", "preview"];

// ── Types ─────────────────────────────────
interface Limits {
  pages: number;
  files: number;
  minKb: number;
}

// ── State ─────────────────────────────────
/** The packs last read, so the view opens complete; they're read again each time it opens. */
let lastPacks: Packs | null = null;

// ── Public API ────────────────────────────
/** Hunt */
export function HuntView() {
  const { hunt, start, stop } = useHunt();
  const [packs, setPacks] = useState<Packs | null>(lastPacks);
  const [topic, setTopic] = useState("");
  const [packId, setPackId] = useState("general");
  const [web, setWeb] = useState(true);
  // `null`: every site of the pack.
  const [sites, setSites] = useState<string[] | null>(null);
  const [kinds, setKinds] = useState<ImageKind[]>(FIRST_KINDS);
  const [limits, setLimits] = useState<Limits>({ pages: 10, files: 200, minKb: 20 });
  const [duplicates, setDuplicates] = useState(false);

  const shown = useViewShown();

  // Read again each time Hunt shows: packs are files the user may have changed.
  // biome-ignore lint/correctness/useExhaustiveDependencies: `shown` is there to read again when Hunt shows
  useEffect(() => {
    huntPacks()
      .then((read) => {
        lastPacks = read;
        setPacks(read);
      })
      .catch(() => {});
  }, [shown]);

  const pack = packs?.packs.find((one) => one.id === packId) ?? packs?.packs[0];
  const picked = sites ?? pack?.sites ?? [];
  const running = hunt.state === "running";
  const progress = hunt.state === "failed" || hunt.state === "idle" ? null : hunt.progress;
  const searchesWeb = web && (pack?.web ?? false);
  const ready = topic.trim() !== "" && pack != null && (searchesWeb || picked.length > 0) && kinds.length > 0;

  const run = () => {
    if (!ready || running || !pack) return;
    void start({ query: topic.trim(), pack: pack.id, web: searchesWeb, sites: picked, kinds, ...limits });
  };

  const choosePack = (id: string | null) => {
    setPackId(id ?? "general");
    setSites(null);
    setWeb(true);
  };

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

  if (duplicates && hunt.state === "done") {
    return (
      <div className="hunt-duplicates">
        <DuplicatesPanel folder={hunt.folder} onClose={() => setDuplicates(false)} />
      </div>
    );
  }

  return (
    <div className="hunt">
      {packs && packs.problems.length > 0 && (
        <Notice tone="warning" title="Some packs couldn't be read">
          {packs.problems.map((problem) => (
            <div key={problem} className="selectable">
              {problem}
            </div>
          ))}
        </Notice>
      )}

      <Panel as="section" className="hunt-card" aria-label="Hunt">
        <div className="hunt-top">
          <Field label="Topic" error={hunt.state === "failed" ? hunt.message : undefined}>
            <TextField
              value={topic}
              onChange={(event) => setTopic(event.target.value)}
              onKeyDown={(event) => event.key === "Enter" && run()}
              spellCheck={false}
              autoComplete="off"
              disabled={running}
              start={<Icon icon={SearchIcon} />}
            />
          </Field>
          <Field label="Pack">
            <Select
              options={(packs?.packs ?? []).map((one) => ({ value: one.id, label: one.name }))}
              value={pack?.id ?? null}
              onValueChange={choosePack}
              disabled={running || !packs}
            />
          </Field>
        </div>
        {pack && (
          <Field label="Search">
            <div className="hunt-sites">
              {pack.web && <Checkbox label="The whole web" checked={web} onCheckedChange={setWeb} disabled={running} />}
              {pack.sites.map((site) => (
                <Checkbox
                  key={site}
                  label={site}
                  checked={picked.includes(site)}
                  disabled={running}
                  onCheckedChange={(on) => setSites(on ? [...picked, site] : picked.filter((one) => one !== site))}
                />
              ))}
            </div>
          </Field>
        )}
        <Field label="Image kinds">
          <div className="hunt-kinds">
            {KINDS.map(({ kind, label }) => (
              <Switch
                key={kind}
                size="sm"
                label={label}
                checked={kinds.includes(kind)}
                disabled={running}
                onCheckedChange={(on) => setKinds(on ? [...kinds, kind] : kinds.filter((one) => one !== kind))}
              />
            ))}
          </div>
        </Field>
        <div className="scrape-limits">
          {limit("pages", "Pages per search", 1, 50)}
          {limit("files", "Images", 1, 5000, 10)}
          {limit("minKb", "Smallest (KB)", 0, 100_000, 5)}
        </div>
        <div className="scrape-actions">
          {running ? (
            <Button icon={<Icon icon={StopIcon} />} onClick={stop}>
              Stop
            </Button>
          ) : (
            <Button variant="primary" icon={<Icon icon={SearchIcon} />} disabled={!ready} onClick={run}>
              Hunt
            </Button>
          )}
        </div>
      </Panel>

      {hunt.state !== "idle" && hunt.state !== "failed" && (
        <Panel as="section" className="hunt-status" aria-label="Hunt progress">
          {running && (
            <ProgressBar label="Images saved" value={progress?.total ? progress.done / progress.total : null} />
          )}
          <div className="scrape-counts">
            <span>
              {hunt.state === "done" ? "Done" : hunt.state === "stopped" ? "Stopped" : "Hunting"}
              {progress
                ? `${hunt.state === "running" && progress.message ? ` · ${progress.message}` : ""} · ${formatCount(progress.done, "image")} saved`
                : " · starting"}
            </span>
            <span className="scrape-folder selectable">{hunt.folder}</span>
            <ShowInFolderButton path={hunt.folder} />
            {hunt.state === "done" && (progress?.done ?? 0) > 1 && (
              <Button size="sm" icon={<Icon icon={DuplicateIcon} />} onClick={() => setDuplicates(true)}>
                Find duplicates
              </Button>
            )}
          </div>
        </Panel>
      )}
    </div>
  );
}

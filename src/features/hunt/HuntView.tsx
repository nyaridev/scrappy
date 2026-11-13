// HuntView: images on a topic from across the web: the topic, the pack (edited, made, removed and hidden in
// `PackEditor`, ADR-0028), which image search engines and which of the pack's sites to search (a closed section until
// opened), the kinds of image a site gives as a tray, each saying what it is on hover, the limits per source and the
// most images they add up to, removing duplicates at the end, start and stop, and the hunt's progress with each
// source's (`HuntProgress`).

import {
  Button,
  Checkbox,
  Collapsible,
  Field,
  Icon,
  IconButton,
  MultiSegmented,
  Notice,
  NumberField,
  Panel,
  Select,
  TextField,
} from "iro-ui";
import type { IconData } from "iro-ui/icons";
import { CodeIcon, ImageIcon, LayersIcon, LinkIcon, PencilIcon, SearchIcon, ShareIcon, ZoomInIcon } from "iro-ui/icons";
import { useEffect, useState } from "react";
import { useSettings } from "../../app/SettingsProvider";
import { useViewShown } from "../../app/viewShown";
import { DuplicatesPanel } from "../../components/DuplicatesPanel";
import { StopButton } from "../../components/StopButton";
import { TipLabel } from "../../components/TipLabel";
import { huntPacks } from "../../lib/backend";
import { formatCount } from "../../lib/format";
import { IMAGE_ENGINES } from "../../lib/imageEngines";
import type { ImageEngine } from "../../lib/types/ImageEngine";
import type { ImageKind } from "../../lib/types/ImageKind";
import type { Packs } from "../../lib/types/Packs";
import { HuntProgress } from "./HuntProgress";
import { PackEditor, YOUR_PACKS } from "./PackEditor";
import { useHunt } from "./useHunt";
import "./hunt.css";

// ── Constants ─────────────────────────────
/** The kinds of image a site's pages give, each with what it is. */
const KINDS: { kind: ImageKind; label: string; icon: IconData; tip: string }[] = [
  { kind: "img", label: "Images", icon: ImageIcon, tip: "Pictures shown in the page's content." },
  {
    kind: "responsive",
    label: "High-res",
    icon: ZoomInIcon,
    tip: "The larger and lazy-loaded versions a page has of its pictures. The largest is kept.",
  },
  { kind: "linked", label: "Full-size", icon: LinkIcon, tip: "Links around pictures that open the full-size file." },
  { kind: "background", label: "Backgrounds", icon: LayersIcon, tip: "Pictures set as a background in the content." },
  { kind: "preview", label: "Previews", icon: ShareIcon, tip: "The picture a page shows when it's shared (og:image)." },
  {
    kind: "inline",
    label: "Inline",
    icon: CodeIcon,
    tip: "Pictures written into the page itself: SVG drawings and data: images. Mostly icons and logos.",
  },
];
const FIRST_KINDS: ImageKind[] = ["img", "responsive", "linked", "background", "preview"];

// ── Types ─────────────────────────────────
interface Limits {
  pages: number;
  perSource: number;
  minKb: number;
}

// ── State ─────────────────────────────────
/** The packs last read, so the view opens complete; they're read again each time it opens. */
let lastPacks: Packs | null = null;

// ── Public API ────────────────────────────
/** Hunt */
export function HuntView() {
  const { hunt, start, stop } = useHunt();
  const starts = useSettings().settings?.preferences?.starts.hunt;
  const [packs, setPacks] = useState<Packs | null>(lastPacks);
  const [topic, setTopic] = useState("");
  const [packId, setPackId] = useState("general");
  const [web, setWeb] = useState(true);
  // `null`: every site of the pack.
  const [sites, setSites] = useState<string[] | null>(null);
  const [engines, setEngines] = useState<ImageEngine[]>(
    () => starts?.engines ?? IMAGE_ENGINES.map(({ engine }) => engine),
  );
  const [kinds, setKinds] = useState<ImageKind[]>(FIRST_KINDS);
  const [removeDuplicates, setRemoveDuplicates] = useState(true);
  const [limits, setLimits] = useState<Limits>(() => ({
    pages: starts?.pages ?? 10,
    perSource: starts?.perSource ?? 50,
    minKb: starts?.minKb ?? 20,
  }));
  const [duplicates, setDuplicates] = useState(false);
  const [editing, setEditing] = useState(false);
  const { settings, saveHiddenPacks } = useSettings();
  const hidden = settings?.hiddenPacks ?? [];

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

  // The Pack list leaves hidden packs out; a picked pack that's hidden gives way to the first shown one.
  const shownPacks = (packs?.packs ?? []).filter((one) => !hidden.includes(one.id));
  const pack = shownPacks.find((one) => one.id === packId) ?? shownPacks[0];
  // Until the user ticks or unticks one: the pack's sites that are switched on.
  const picked = sites ?? pack?.sites.filter((site) => !pack.off.includes(site)) ?? [];
  const running = hunt.state === "running";
  const searchesWeb = web && (pack?.web ?? false);
  const pageSources = picked.length + (searchesWeb ? 1 : 0);
  // The most a hunt saves: each source stops at its limit.
  const most = (engines.length + pageSources) * limits.perSource;
  // Engines give whole images: only sites need a kind of image picked.
  const ready =
    topic.trim() !== "" && pack != null && engines.length + pageSources > 0 && (pageSources === 0 || kinds.length > 0);

  const run = () => {
    if (!ready || running || !pack) return;
    void start({
      query: topic.trim(),
      pack: pack.id,
      web: searchesWeb,
      sites: picked,
      engines,
      kinds,
      removeDuplicates,
      ...limits,
    });
  };

  const choosePack = (id: string | null) => {
    setPackId(id ?? "general");
    setSites(null);
    setWeb(true);
  };

  const saved = (read: Packs) => {
    lastPacks = read;
    setPacks(read);
    setSites(null);
    setWeb(true);
    setEditing(false);
  };

  // A new pack is picked; an edited one leaves the pick as it was.
  const savedPack = (read: Packs, id: string, created: boolean) => {
    saved(read);
    if (created) setPackId(id);
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
      <div className="duplicates-page">
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
            <div className="hunt-pack">
              <Select
                className="hunt-pack-select"
                options={shownPacks.map((one) => ({ value: one.id, label: one.name, group: one.group ?? YOUR_PACKS }))}
                value={pack?.id ?? null}
                onValueChange={choosePack}
                disabled={running || !packs}
              />
              <IconButton
                icon={<Icon icon={PencilIcon} />}
                label="Edit packs"
                disabled={running || !packs}
                onClick={() => setEditing(true)}
              />
            </div>
          </Field>
        </div>
        {pack && (
          <Collapsible tone="inset" title="Sources" summary={`${engines.length + pageSources} picked`}>
            <div className="hunt-sites">
              {IMAGE_ENGINES.map(({ engine, label }) => (
                <Checkbox
                  key={engine}
                  label={label}
                  checked={engines.includes(engine)}
                  disabled={running}
                  onCheckedChange={(on) =>
                    setEngines(on ? [...engines, engine] : engines.filter((one) => one !== engine))
                  }
                />
              ))}
              {pack.web && (
                <Checkbox label="Any site (web search)" checked={web} onCheckedChange={setWeb} disabled={running} />
              )}
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
          </Collapsible>
        )}
        <Field label="Image kinds">
          <MultiSegmented
            className="fill-segments"
            aria-label="Image kinds"
            options={KINDS.map(({ kind, label, icon, tip }) => ({
              value: kind,
              label: <TipLabel icon={icon} label={label} tip={tip} />,
            }))}
            value={kinds}
            onValueChange={setKinds}
            disabled={running}
          />
        </Field>
        <div className="scrape-limits">
          {limit("pages", "Pages per source", 1, 50)}
          {limit("perSource", "Images per source", 1, 1000, 10)}
          {limit("minKb", "Smallest (KB)", 0, 100_000, 5)}
        </div>
        <div className="scrape-actions">
          <Checkbox
            className="hunt-dedupe"
            label="Remove duplicates"
            description="Keeps the best copy of each picture; the rest go to the trash."
            checked={removeDuplicates}
            disabled={running}
            onCheckedChange={setRemoveDuplicates}
          />
          <span className="hunt-estimate">Up to {formatCount(most, "image")}</span>
          {running ? (
            <StopButton onStop={stop} />
          ) : (
            <Button variant="primary" icon={<Icon icon={SearchIcon} />} disabled={!ready} onClick={run}>
              Hunt
            </Button>
          )}
        </div>
      </Panel>

      {editing && packs && (
        <PackEditor
          packs={packs.packs}
          first={pack?.id ?? ""}
          hidden={hidden}
          onHidden={saveHiddenPacks}
          onClose={() => setEditing(false)}
          onSaved={savedPack}
          onRemoved={saved}
        />
      )}

      {hunt.state !== "idle" && (hunt.state !== "failed" || hunt.status.sources.length > 0) && (
        <HuntProgress hunt={hunt} onDuplicates={() => setDuplicates(true)} />
      )}
    </div>
  );
}

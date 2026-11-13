// StartSettings: the Views tab of Settings: one card per view with what it starts with when it opens (Grab, Scrape,
// Hunt, with Hunt's packs folder), and how alike pictures must be when Grab and Hunt remove copies on their own
// (ADR-0028, ADR-0034).

import { Button, Checkbox, Field, Icon, NumberField, Panel, SectionTitle, Segmented, Switch, toast } from "iro-ui";
import { DuplicateIcon, FolderOpenIcon, GlobeIcon, SearchIcon, TargetIcon } from "iro-ui/icons";
import { useSettings } from "../../app/SettingsProvider";
import { openPacksFolder } from "../../lib/backend";
import { IMAGE_ENGINES } from "../../lib/imageEngines";
import type { StartValues } from "../../lib/types/StartValues";
import type { Strictness } from "../../lib/types/Strictness";

// ── Constants ─────────────────────────────
const STRICTNESS: { value: Strictness; label: string }[] = [
  { value: "strict", label: "Strict" },
  { value: "normal", label: "Normal" },
  { value: "loose", label: "Loose" },
];

// ── Private helpers ───────────────────────
function openPacks() {
  openPacksFolder().catch((error) =>
    toast({ tone: "error", title: "Couldn't open the folder", description: String(error) }),
  );
}

// ── Public API ────────────────────────────
/** The Views tab's cards. */
export function StartSettings() {
  const { settings, savePreferences } = useSettings();
  const preferences = settings?.preferences;
  if (!preferences) return null;
  const { grab, scrape, hunt } = preferences.starts;
  const save = (starts: Partial<StartValues>) =>
    void savePreferences({ ...preferences, starts: { ...preferences.starts, ...starts } });

  /** A number field for one starting value. */
  const number = (label: string, value: number, min: number, max: number, set: (value: number) => void, step = 1) => (
    <Field label={label}>
      <NumberField value={value} min={min} max={max} step={step} onValueChange={(next) => next != null && set(next)} />
    </Field>
  );

  return (
    <>
      <p className="settings-hint">What each view starts with when it opens.</p>

      <Panel as="section" className="settings-section">
        <SectionTitle icon={<Icon icon={TargetIcon} />}>Grab</SectionTitle>
        <div className="settings-fields">
          {number("Smallest (KB)", grab.minKb, 0, 100_000, (minKb) => save({ grab: { ...grab, minKb } }), 5)}
        </div>
        <Switch
          label="Remove duplicates"
          checked={grab.removeDuplicates}
          onCheckedChange={(removeDuplicates) => save({ grab: { ...grab, removeDuplicates } })}
        />
      </Panel>

      <Panel as="section" className="settings-section">
        <SectionTitle icon={<Icon icon={GlobeIcon} />}>Scrape</SectionTitle>
        <div className="settings-fields">
          {number("Depth", scrape.depth, 1, 10, (depth) => save({ scrape: { ...scrape, depth } }))}
          {number("Pages", scrape.pages, 1, 10_000, (pages) => save({ scrape: { ...scrape, pages } }), 10)}
          <Field label="Delay (s)">
            <NumberField
              value={scrape.delayMs / 1000}
              min={0}
              max={60}
              step={0.1}
              format={{ maximumFractionDigits: 1 }}
              onValueChange={(value) => save({ scrape: { ...scrape, delayMs: Math.round((value ?? 0) * 1000) } })}
            />
          </Field>
          {number(
            "Max video size (MB)",
            scrape.maxVideoMb,
            1,
            100_000,
            (maxVideoMb) => save({ scrape: { ...scrape, maxVideoMb } }),
            50,
          )}
        </div>
      </Panel>

      <Panel as="section" className="settings-section">
        <SectionTitle
          icon={<Icon icon={SearchIcon} />}
          actions={
            <Button size="sm" icon={<Icon icon={FolderOpenIcon} />} onClick={openPacks}>
              Open packs folder
            </Button>
          }
        >
          Hunt
        </SectionTitle>
        <div className="settings-fields">
          {number("Pages per source", hunt.pages, 1, 50, (pages) => save({ hunt: { ...hunt, pages } }))}
          {number(
            "Images per source",
            hunt.perSource,
            1,
            1000,
            (perSource) => save({ hunt: { ...hunt, perSource } }),
            10,
          )}
          {number("Smallest (KB)", hunt.minKb, 0, 100_000, (minKb) => save({ hunt: { ...hunt, minKb } }), 5)}
        </div>
        <Field label="Image search engines">
          <div className="settings-switches">
            {IMAGE_ENGINES.map(({ engine, label }) => (
              <Checkbox
                key={engine}
                label={label}
                checked={hunt.engines.includes(engine)}
                onCheckedChange={(on) =>
                  save({
                    hunt: {
                      ...hunt,
                      engines: on ? [...hunt.engines, engine] : hunt.engines.filter((one) => one !== engine),
                    },
                  })
                }
              />
            ))}
          </div>
        </Field>
        <p className="settings-hint">A pack file in the packs folder with a built-in pack's name replaces it.</p>
      </Panel>

      <Panel as="section" className="settings-section">
        <SectionTitle icon={<Icon icon={DuplicateIcon} />}>Removing duplicates</SectionTitle>
        <Field label="Matches" hint="How alike pictures must be when Grab and Hunt remove copies.">
          <Segmented
            full
            options={STRICTNESS}
            value={preferences.duplicates}
            onValueChange={(duplicates) => void savePreferences({ ...preferences, duplicates })}
          />
        </Field>
      </Panel>
    </>
  );
}

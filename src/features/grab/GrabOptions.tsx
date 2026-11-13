// GrabOptions: how far a scan goes and what it saves on its own (ADR-0031, ADR-0032): the depth (up to 10), the
// smallest size shown (and kept), and saving finds as they're found: the kinds kept as a tray of toggles, the most
// files, and removing duplicates after.

import { Field, MultiSegmented, NumberField, Switch } from "iro-ui";
import { FIND_KINDS } from "../../components/finds/findKinds";
import { TipLabel } from "../../components/TipLabel";
import type { FindKind } from "../../lib/types/FindKind";
import type { ScanRequest } from "../../lib/types/ScanRequest";

// ── Types ─────────────────────────────────
/** A scan request without its link. */
export type ScanOptions = Omit<ScanRequest, "url">;

// ── Constants ─────────────────────────────
export const SCAN_DEFAULTS: ScanOptions = {
  depth: 1,
  save: false,
  kinds: ["image"],
  minKb: 10,
  files: 200,
  removeDuplicates: true,
};

/** The deepest scan; Rust holds to it too. */
const MAX_DEPTH = 10;

/** What saving as found can keep; streams go through yt-dlp, so they're picked. */
const KEEP: { value: FindKind; label: string }[] = [
  { value: "image", label: "Images" },
  { value: "video", label: "Videos" },
  { value: "audio", label: "Audio" },
  { value: "other", label: "Other files" },
];

// ── Public API ────────────────────────────
/** The options under the page's link; `scanning` locks those a running scan can't change. */
export function GrabOptions({
  options,
  onChange,
  scanning,
}: {
  options: ScanOptions;
  onChange: (options: ScanOptions) => void;
  scanning: boolean;
}) {
  const set = (part: Partial<ScanOptions>) => onChange({ ...options, ...part });
  return (
    <>
      <div className="scrape-limits">
        <Field label="Depth">
          <NumberField
            value={options.depth}
            min={1}
            max={MAX_DEPTH}
            disabled={scanning}
            onValueChange={(depth) => set({ depth: depth ?? 1 })}
          />
        </Field>
        <Field label="Smallest (KB)">
          <NumberField
            value={options.minKb}
            min={0}
            max={100_000}
            step={5}
            onValueChange={(minKb) => set({ minKb: minKb ?? 0 })}
          />
        </Field>
      </div>
      <div className="grab-save">
        <Switch
          label="Download as found"
          checked={options.save}
          disabled={scanning}
          onCheckedChange={(save) => set({ save })}
        />
        {options.save && (
          <Switch
            label="Remove duplicates"
            checked={options.removeDuplicates}
            disabled={scanning}
            onCheckedChange={(removeDuplicates) => set({ removeDuplicates })}
          />
        )}
      </div>
      {options.save && (
        <div className="grab-keep">
          <Field label="Keep">
            <MultiSegmented
              className="fill-segments"
              aria-label="Keep"
              options={KEEP.map(({ value, label }) => ({
                value,
                label: <TipLabel icon={FIND_KINDS[value].icon} label={label} tip={FIND_KINDS[value].about} />,
              }))}
              value={options.kinds}
              onValueChange={(kinds) => set({ kinds })}
              disabled={scanning}
            />
          </Field>
          <Field label="Files">
            <NumberField
              value={options.files}
              min={1}
              max={10_000}
              step={10}
              disabled={scanning}
              onValueChange={(files) => set({ files: files ?? 1 })}
            />
          </Field>
        </div>
      )}
    </>
  );
}

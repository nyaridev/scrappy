// DownloadView: paste a link, Rust checks it (Enter or the arrow) and shows what's there until the next check; pick
// a preset and quality (for a playlist or profile, how many; for a playlist or channel, which lists and videos),
// download. A single video shows the size it will download. The kept downloads are listed below, with Clear all.

import { Button, Field, Icon, IconButton, Panel, Select, TextField } from "iro-ui";
import { ArrowRightIcon, DownloadIcon, LinkIcon } from "iro-ui/icons";
import { useEffect, useState } from "react";
import { useCached } from "../../app/useCached";
import { downloadSize, presets, probeUrl } from "../../lib/backend";
import { formatBytes } from "../../lib/format";
import type { Probe } from "../../lib/types/Probe";
import type { Section } from "../../lib/types/Section";
import { DownloadQueue } from "./DownloadQueue";
import { ListTiles } from "./ListTiles";
import { ProbeCard } from "./ProbeCard";
import { useDownloads } from "./useDownloads";

// ── Constants ─────────────────────────────
const BEST = "best";
const ALL = "all";
/** The "First N" choices for a playlist or profile. */
const AMOUNTS = [10, 25, 50, 100];

// ── Private helpers ───────────────────────
/** The lists to show as tiles: a channel's sections, or a playlist as one list; none for anything else. */
function listsOf(probe: Probe | null): Section[] {
  if (!probe) return [];
  if (probe.sections.length > 0) return probe.sections;
  if (probe.files || probe.count == null) return [];
  return [{ url: probe.url, title: "Videos", count: probe.count, more: probe.more, entries: probe.entries }];
}

// ── Public API ────────────────────────────
/** Download */
export function DownloadView() {
  const [link, setLink] = useState("");
  // The link as it was checked: Enter downloads while the field still holds it, and checks again once it changes.
  const [checkedLink, setCheckedLink] = useState("");
  const [probe, setProbe] = useState<Probe | null>(null);
  const [checking, setChecking] = useState(false);
  const [linkError, setLinkError] = useState<string | null>(null);
  const [allPresets] = useCached("presets", presets);
  const [presetId, setPresetId] = useState<string | null>(null);
  const [quality, setQuality] = useState(BEST);
  const [amount, setAmount] = useState(ALL);
  const [starting, setStarting] = useState(false);
  const [sections, setSections] = useState<string[]>([]);
  const [skip, setSkip] = useState<string[]>([]);
  const [size, setSize] = useState<number | null>(null);
  const { entries, progress, start, cancel, remove, clear } = useDownloads();

  const preset = allPresets?.find((p) => p.id === presetId) ?? allPresets?.[0] ?? null;
  const maxHeight = quality === BEST || preset?.audio ? null : Number(quality);
  const qualityOptions = [
    { value: BEST, label: "Best" },
    ...(probe?.heights ?? []).map((height) => ({ value: String(height), label: `${height}p` })),
  ];
  const lists = listsOf(probe);
  const hasSections = (probe?.sections.length ?? 0) > 0;
  const picked = hasSections ? lists.filter((list) => sections.includes(list.url)) : lists;
  const skipped = new Set(skip);
  const count =
    lists.length > 0
      ? picked.reduce((sum, list) => sum + list.count - list.entries.filter((e) => skipped.has(e.url)).length, 0)
      : (probe?.count ?? null);
  const more = lists.length > 0 ? picked.some((list) => list.more) : probe?.more;
  const finished = entries.some((entry) => entry.state !== "queued" && entry.state !== "running");
  const amountOptions =
    count == null
      ? []
      : [
          { value: ALL, label: more ? "All" : `All ${count}` },
          ...AMOUNTS.filter((n) => more || n < count).map((n) => ({ value: String(n), label: `First ${n}` })),
        ];
  const downloadLabel = count == null ? "Download" : amount === ALL ? "Download all" : `Download ${amount}`;

  // Only a single video has a size before it downloads: playlist and profile listings carry none.
  useEffect(() => {
    setSize(null);
    if (!probe || probe.count != null || probe.files || !preset) return;
    let current = true;
    void downloadSize(probe.url, preset.id, maxHeight)
      .catch(() => null)
      .then((bytes) => current && setSize(bytes));
    return () => {
      current = false;
    };
  }, [probe, preset, maxHeight]);

  async function check() {
    const url = link.trim();
    if (!url || checking) return;
    setChecking(true);
    setLinkError(null);
    setProbe(null);
    try {
      const found = await probeUrl(url);
      setProbe(found);
      setCheckedLink(url);
      setSections(found.sections.map((section) => section.url));
      setSkip([]);
      setQuality(BEST);
      setAmount(ALL);
    } catch (e) {
      setLinkError(String(e));
    } finally {
      setChecking(false);
    }
  }

  async function download() {
    if (!probe || !preset) return;
    setStarting(true);
    setLinkError(null);
    try {
      await start({
        url: probe.url,
        preset: preset.id,
        maxHeight,
        limit: amount === ALL ? null : Number(amount),
        sections: hasSections ? sections : [],
        skip,
      });
      setLink("");
      setProbe(null);
    } catch (e) {
      setLinkError(String(e));
    } finally {
      setStarting(false);
    }
  }

  return (
    <div className="download">
      <Panel as="section" className="download-card" aria-label="New download">
        <Field label="Link" error={linkError ?? undefined}>
          <TextField
            value={link}
            onChange={(event) => setLink(event.target.value)}
            onKeyDown={(event) =>
              event.key === "Enter" && void (probe && link.trim() === checkedLink ? download() : check())
            }
            placeholder="https://"
            spellCheck={false}
            autoComplete="off"
            start={<Icon icon={LinkIcon} />}
            end={
              <IconButton
                icon={<Icon icon={ArrowRightIcon} />}
                label="Check link"
                loading={checking}
                disabled={!link.trim()}
                onClick={check}
              />
            }
          />
        </Field>

        {probe && (
          <>
            <ProbeCard probe={probe} showItems={lists.length === 0} />
            {lists.length > 0 && (
              <ListTiles
                lists={lists}
                on={hasSections ? sections : null}
                onOnChange={setSections}
                skip={skip}
                onSkipChange={setSkip}
              />
            )}
            <div className="download-options">
              {count != null && (
                <Field label="Amount" hint={picked.length > 1 ? "Of each list" : undefined}>
                  <Select options={amountOptions} value={amount} onValueChange={(v) => setAmount(v ?? ALL)} />
                </Field>
              )}
              {!probe.files && (
                <Field label="Preset">
                  <Select
                    options={(allPresets ?? []).map((p) => ({
                      value: p.id,
                      label: p.name,
                      group: p.audio ? "Audio" : "Video",
                    }))}
                    value={preset?.id ?? null}
                    onValueChange={setPresetId}
                  />
                </Field>
              )}
              {!probe.files && !preset?.audio && probe.heights.length > 0 && (
                <Field label="Quality">
                  <Select options={qualityOptions} value={quality} onValueChange={(v) => setQuality(v ?? BEST)} />
                </Field>
              )}
            </div>
            <div className="download-actions">
              {size != null && <span className="download-size">About {formatBytes(size)}</span>}
              <Button
                variant="primary"
                icon={<Icon icon={DownloadIcon} />}
                loading={starting}
                disabled={!preset || count === 0}
                onClick={download}
              >
                {downloadLabel}
              </Button>
            </div>
          </>
        )}
      </Panel>

      {entries.length > 0 && (
        <section className="downloads" aria-label="Downloads">
          <div className="downloads-head">
            <Button size="sm" variant="ghost" disabled={!finished} onClick={clear}>
              Clear all
            </Button>
          </div>
          <DownloadQueue entries={entries} progress={progress} onCancel={cancel} onRemove={remove} />
        </section>
      )}
    </div>
  );
}

// DownloadView: paste a link, Rust checks it (Enter or the arrow) and shows what's there until the next check; a list
// shows from its first videos and grows in place until it's listed or stopped (ADR-0030). Pick a preset and quality,
// starting on those Settings names (ADR-0034) (for a playlist or profile, how many; for a playlist or channel, which lists and videos), download. A single video
// shows the size it will download. Its ten latest downloads are listed below under their heading; every download is
// in the Downloads view (ADR-0018).

import { Button, Field, Icon, IconButton, Panel, SectionTitle, Select, TextField } from "iro-ui";
import { ArrowRightIcon, DownloadIcon, HistoryIcon, LinkIcon } from "iro-ui/icons";
import { useEffect, useMemo, useState } from "react";
import { DOWNLOADS, useOpenView } from "../../app/openView";
import { useSettings } from "../../app/SettingsProvider";
import { useCached } from "../../app/useCached";
import { useDownloads } from "../../app/useDownloads";
import { DownloadQueue } from "../../components/DownloadQueue";
import { StopButton } from "../../components/StopButton";
import { downloadSize, presets } from "../../lib/backend";
import { formatBytes } from "../../lib/format";
import type { Probe } from "../../lib/types/Probe";
import type { Section } from "../../lib/types/Section";
import { ListTiles } from "./ListTiles";
import { ProbeCard } from "./ProbeCard";
import { useLinkCheck } from "./useLinkCheck";
import "./download.css";

// ── Constants ─────────────────────────────
const BEST = "best";
const ALL = "all";
/** The "First N" choices for a playlist or profile. */
const AMOUNTS = [10, 25, 50, 100, 150, 200, 300, 500];
/** How many of this view's downloads it lists. */
const LATEST = 10;

// ── Private helpers ───────────────────────
/** While listing, a list with no count of its own is counted as listed so far: more may come. */
function growing(list: { count: number | null; entries: unknown[] }, listing: boolean): boolean {
  return listing && list.count != null && list.entries.length >= list.count;
}

/** The lists to show as tiles: a channel's sections, or a playlist as one list; none for anything else. */
function listsOf(probe: Probe | null, listing: boolean): Section[] {
  if (!probe) return [];
  const lists =
    probe.sections.length > 0 || probe.files || probe.count == null
      ? probe.sections
      : [{ url: probe.url, title: "Videos", count: probe.count, more: probe.more, entries: probe.entries }];
  return lists.map((list) => ({ ...list, more: list.more || growing(list, listing) }));
}

/** The quality a link starts on: the highest it has up to `most` (Settings), else Best. */
function startQuality(heights: number[], most: number | null): string {
  const fits = most == null ? [] : heights.filter((height) => height <= most);
  return fits.length > 0 ? String(Math.max(...fits)) : BEST;
}

// ── Public API ────────────────────────────
/** Download */
export function DownloadView() {
  const [link, setLink] = useState("");
  // The link as it was checked: Enter downloads while the field still holds it, and checks again once it changes.
  const [checkedLink, setCheckedLink] = useState("");
  const { probe, running, error: checkError, check: checkLink, stop, clear } = useLinkCheck();
  const [linkError, setLinkError] = useState<string | null>(null);
  const [allPresets] = useCached("presets", presets);
  // `null`: what Settings starts on.
  const [presetId, setPresetId] = useState<string | null>(null);
  const [chosenQuality, setQuality] = useState<string | null>(null);
  const media = useSettings().settings?.preferences?.media;
  const [amount, setAmount] = useState(ALL);
  const [starting, setStarting] = useState(false);
  // A channel's lists turned off; lists that show up while listing start on.
  const [sectionsOff, setSectionsOff] = useState<string[]>([]);
  const [skip, setSkip] = useState<string[]>([]);
  const [size, setSize] = useState<number | null>(null);
  const { entries, progress, start, cancel, remove } = useDownloads();
  const openView = useOpenView();
  const latest = useMemo(() => entries.filter((entry) => entry.source === "download").slice(0, LATEST), [entries]);

  const listing = running && probe != null;
  const preset = allPresets?.find((p) => p.id === (presetId ?? media?.preset)) ?? allPresets?.[0] ?? null;
  const quality = chosenQuality ?? startQuality(probe?.heights ?? [], media?.maxHeight ?? null);
  const maxHeight = quality === BEST || preset?.audio ? null : Number(quality);
  const qualityOptions = [
    { value: BEST, label: "Best" },
    ...(probe?.heights ?? []).map((height) => ({ value: String(height), label: `${height}p` })),
  ];
  const lists = listsOf(probe, listing);
  const hasSections = (probe?.sections.length ?? 0) > 0;
  const sections = hasSections ? lists.map((list) => list.url).filter((url) => !sectionsOff.includes(url)) : [];
  const picked = hasSections ? lists.filter((list) => sections.includes(list.url)) : lists;
  const skipped = new Set(skip);
  const count =
    lists.length > 0
      ? picked.reduce((sum, list) => sum + list.count - list.entries.filter((e) => skipped.has(e.url)).length, 0)
      : (probe?.count ?? null);
  const more = lists.length > 0 ? picked.some((list) => list.more) : probe?.more;
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

  function check(url = link.trim()) {
    if (!url || (running && !probe)) return;
    setLinkError(null);
    setCheckedLink(url);
    setSectionsOff([]);
    setSkip([]);
    setQuality(null);
    setAmount(ALL);
    void checkLink(url);
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
        sections,
        skip,
      });
      setLink("");
      // A download started while listing doesn't need the rest of the list (ADR-0030).
      clear();
    } catch (e) {
      setLinkError(String(e));
    } finally {
      setStarting(false);
    }
  }

  return (
    <div className="download">
      <Panel as="section" className="download-card" aria-label="New download">
        <Field label="Link" error={linkError ?? checkError ?? undefined}>
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
                loading={running && !probe}
                disabled={!link.trim()}
                onClick={() => check()}
              />
            }
          />
        </Field>

        {probe && (
          <>
            <ProbeCard
              probe={{ ...probe, more: probe.more || lists.some((list) => list.more) }}
              showItems={lists.length === 0}
            />
            {lists.length > 0 && (
              <ListTiles
                lists={lists}
                on={hasSections ? sections : null}
                onOnChange={(on) => setSectionsOff(lists.map((list) => list.url).filter((url) => !on.includes(url)))}
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
              <span className="download-start">
                {listing && <StopButton onStop={stop} />}
                {probe.video && (
                  <Button
                    variant="ghost"
                    onClick={() => {
                      setLink(probe.video ?? "");
                      check(probe.video ?? "");
                    }}
                  >
                    Just this video
                  </Button>
                )}
              </span>
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

      {latest.length > 0 && (
        <section className="downloads" aria-labelledby="download-latest">
          <div className="downloads-head">
            <SectionTitle id="download-latest" icon={<Icon icon={HistoryIcon} />}>
              {latest.length === 1 ? "Last download" : `Last ${latest.length} downloads`}
            </SectionTitle>
            <Button size="sm" variant="ghost" onClick={() => openView(DOWNLOADS)}>
              All downloads
            </Button>
          </div>
          <DownloadQueue entries={latest} progress={progress} onCancel={cancel} onRemove={remove} />
        </section>
      )}
    </div>
  );
}

// DownloadView: paste a link, Rust checks it and shows what's there, pick a preset and quality (and for a playlist
// or profile, how many), download. This session's downloads are listed below.

import { Button, Field, Icon, IconButton, Panel, Select, TextField } from "iro-ui";
import { ArrowRightIcon, DownloadIcon, LinkIcon } from "iro-ui/icons";
import { useState } from "react";
import { useCached } from "../../app/useCached";
import { presets, probeUrl } from "../../lib/backend";
import type { Probe } from "../../lib/types/Probe";
import { DownloadQueue } from "./DownloadQueue";
import { ProbeCard } from "./ProbeCard";
import { useDownloads } from "./useDownloads";

// ── Constants ─────────────────────────────
const BEST = "best";
const ALL = "all";
/** The "First N" choices for a playlist or profile. */
const AMOUNTS = [10, 25, 50, 100];

// ── Public API ────────────────────────────
/** Download */
export function DownloadView() {
  const [link, setLink] = useState("");
  const [probe, setProbe] = useState<Probe | null>(null);
  const [checking, setChecking] = useState(false);
  const [linkError, setLinkError] = useState<string | null>(null);
  const [allPresets] = useCached("presets", presets);
  const [presetId, setPresetId] = useState<string | null>(null);
  const [quality, setQuality] = useState(BEST);
  const [amount, setAmount] = useState(ALL);
  const [starting, setStarting] = useState(false);
  const { entries, progress, start, cancel } = useDownloads();

  const preset = allPresets?.find((p) => p.id === presetId) ?? allPresets?.[0] ?? null;
  const qualityOptions = [
    { value: BEST, label: "Best" },
    ...(probe?.heights ?? []).map((height) => ({ value: String(height), label: `${height}p` })),
  ];
  const count = probe?.count ?? null;
  const amountOptions =
    count == null
      ? []
      : [
          { value: ALL, label: probe?.more ? "All" : `All ${count}` },
          ...AMOUNTS.filter((n) => probe?.more || n < count).map((n) => ({ value: String(n), label: `First ${n}` })),
        ];
  const downloadLabel = count == null ? "Download" : amount === ALL ? "Download all" : `Download ${amount}`;

  async function check() {
    const url = link.trim();
    if (!url || checking) return;
    setChecking(true);
    setLinkError(null);
    setProbe(null);
    try {
      setProbe(await probeUrl(url));
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
        maxHeight: quality === BEST ? null : Number(quality),
        limit: amount === ALL ? null : Number(amount),
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
            onChange={(event) => {
              setLink(event.target.value);
              setProbe(null);
              setLinkError(null);
            }}
            onKeyDown={(event) => event.key === "Enter" && void (probe ? download() : check())}
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
            <ProbeCard probe={probe} />
            <div className="download-options">
              {count != null && (
                <Field label="Amount">
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
              <Button
                variant="primary"
                icon={<Icon icon={DownloadIcon} />}
                loading={starting}
                disabled={!preset}
                onClick={download}
              >
                {downloadLabel}
              </Button>
            </div>
          </>
        )}
      </Panel>

      {entries.length > 0 && <DownloadQueue entries={entries} progress={progress} onCancel={cancel} />}
    </div>
  );
}

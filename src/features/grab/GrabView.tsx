// GrabView: a page's link is scanned with a headless browser, to a depth of linked pages (or the extension hands
// the page over), and everything it can download is shown with thumbnails as the scan finds it (`FindResults`): hide
// small finds, filter by type, sort, pick, download into `Grabs/`, removing duplicates after (ADR-0029). Saving as
// found saves the kept kinds while it scans, each tile showing how its find went (ADR-0031).

import { Button, Checkbox, EmptyState, Field, Icon, IconButton, Panel, Spinner, TextField } from "iro-ui";
import { DownloadIcon, LinkIcon, SearchIcon } from "iro-ui/icons";
import { useMemo, useState } from "react";
import { useSettings } from "../../app/SettingsProvider";
import { FindResults } from "../../components/finds/FindResults";
import { StopButton } from "../../components/StopButton";
import { formatCount } from "../../lib/format";
import { GrabOptions, SCAN_DEFAULTS, type ScanOptions } from "./GrabOptions";
import { useGrab } from "./useGrab";
import "./grab.css";

// ── Private helpers ───────────────────────
function hostOf(url: string): string {
  try {
    return new URL(url).host;
  } catch {
    return url;
  }
}

// ── Public API ────────────────────────────
/** Grab */
export function GrabView() {
  const { page, fresh, picked, scan, stop, grab } = useGrab();
  const [link, setLink] = useState(page?.url ?? "");
  const [scanning, setScanning] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [selected, setSelected] = useState<string[]>([]);
  const [starting, setStarting] = useState(false);
  const [shownFresh, setShownFresh] = useState(fresh);
  const start = useSettings().settings?.preferences?.starts.grab;
  const [scanOptions, setScanOptions] = useState<ScanOptions>(() => ({ ...SCAN_DEFAULTS, ...start }));
  const { minKb, removeDuplicates } = scanOptions;

  // A new page (scanned, or handed over) starts with nothing picked (and in `FindResults`, no filter, on its first
  // page); a scan's updates keep them.
  if (fresh !== shownFresh) {
    setShownFresh(fresh);
    setSelected([]);
    if (page) setLink(page.url);
  }

  const all = page?.finds ?? [];
  // Finds the page said are smaller than "Smallest" are hidden; one of unknown size stays (ADR-0029).
  const finds = useMemo(() => all.filter((find) => find.size == null || find.size >= minKb * 1024), [all, minKb]);
  const small = all.length - finds.length;
  const chosen = useMemo(() => {
    const picked = new Set(selected);
    return finds.filter((find) => picked.has(find.url));
  }, [finds, selected]);
  const saved = Object.values(page?.kept ?? {}).filter((kept) => kept.state === "saved").length;
  const ready = link.trim() !== "" && (!scanOptions.save || scanOptions.kinds.length > 0);

  async function runScan() {
    const url = link.trim();
    if (!ready || scanning) return;
    setScanning(true);
    setError(null);
    try {
      await scan({ url, ...scanOptions });
    } catch (e) {
      setError(String(e));
    } finally {
      setScanning(false);
    }
  }

  async function download() {
    setStarting(true);
    setError(null);
    try {
      await grab(chosen, removeDuplicates);
      setSelected([]);
    } catch (e) {
      setError(String(e));
    } finally {
      setStarting(false);
    }
  }

  return (
    <div className="grab">
      <Panel as="section" className="grab-card" aria-label="Scan a page">
        <Field label="Page" error={error ?? undefined}>
          <TextField
            value={link}
            onChange={(event) => {
              setLink(event.target.value);
              setError(null);
            }}
            onKeyDown={(event) => event.key === "Enter" && void runScan()}
            placeholder="https://"
            spellCheck={false}
            autoComplete="off"
            disabled={scanning}
            start={<Icon icon={LinkIcon} />}
            end={
              <IconButton
                icon={<Icon icon={SearchIcon} />}
                label="Scan page"
                loading={scanning}
                disabled={!ready}
                onClick={runScan}
              />
            }
          />
        </Field>
        <GrabOptions options={scanOptions} onChange={setScanOptions} scanning={scanning} />
      </Panel>

      {page && (
        <Panel as="section" className="finds-panel" aria-label="Found on the page">
          <div className="finds-head">
            <div className="finds-heading">
              <span className="finds-title">{page.title ?? hostOf(page.url)}</span>
              <span className="finds-facts" aria-live="polite">
                {scanning && <Spinner />}
                {[
                  scanning ? "Scanning" : null,
                  page.pages > 1 ? formatCount(page.pages, "page") : null,
                  `${finds.length} found`,
                  saved > 0 ? `${saved} saved` : null,
                  small > 0 ? `${small} under ${minKb} KB` : null,
                  hostOf(page.url),
                ]
                  .filter(Boolean)
                  .join(" · ")}
              </span>
            </div>
            {scanning && <StopButton size="sm" onStop={stop} />}
          </div>
          {finds.length === 0 ? (
            !scanning && (
              <EmptyState
                title={all.length > 0 ? `Everything found is under ${minKb} KB` : "Nothing to download on this page"}
              />
            )
          ) : (
            <FindResults
              finds={finds}
              picked={picked}
              kept={page.kept}
              selected={selected}
              onSelectedChange={setSelected}
              fresh={fresh}
            >
              <Checkbox
                label="Remove duplicates"
                checked={removeDuplicates}
                onCheckedChange={(on) => setScanOptions((all) => ({ ...all, removeDuplicates: on }))}
              />
              <Button
                variant="primary"
                icon={<Icon icon={DownloadIcon} />}
                loading={starting}
                disabled={chosen.length === 0}
                onClick={download}
              >
                {chosen.length > 0 ? `Download ${chosen.length}` : "Download"}
              </Button>
            </FindResults>
          )}
        </Panel>
      )}
    </div>
  );
}

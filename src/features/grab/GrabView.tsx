// GrabView: a page's link is scanned in a headless browser (or the extension hands the page over), and everything it
// can download is listed: filter by type, sort, pick, download into `Grabs/`.

import { Button, EmptyState, Field, Icon, IconButton, MultiSegmented, Panel, Spinner, TextField } from "iro-ui";
import { DownloadIcon, LinkIcon, SearchIcon } from "iro-ui/icons";
import { useEffect, useMemo, useState } from "react";
import type { FindKind } from "../../lib/types/FindKind";
import { FindTable } from "./FindTable";
import { FIND_KINDS } from "./findKinds";
import { useGrab } from "./useGrab";

// ── Constants ─────────────────────────────
const KINDS = Object.keys(FIND_KINDS) as FindKind[];

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
  const { page, picked, scan, grab } = useGrab();
  const [link, setLink] = useState(page?.url ?? "");
  const [scanning, setScanning] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [kinds, setKinds] = useState<FindKind[]>([]);
  const [selected, setSelected] = useState<string[]>([]);
  const [starting, setStarting] = useState(false);

  // A new page (scanned, or handed over) starts with no filter and nothing picked.
  useEffect(() => {
    setKinds([]);
    setSelected([]);
    if (page) setLink(page.url);
  }, [page]);

  const finds = page?.finds ?? [];
  const counts = Object.fromEntries(KINDS.map((kind) => [kind, finds.filter((f) => f.kind === kind).length]));
  const options = KINDS.filter((kind) => counts[kind] > 0).map((kind) => ({
    value: kind,
    label: `${FIND_KINDS[kind].label} ${counts[kind]}`,
    icon: <Icon icon={FIND_KINDS[kind].icon} />,
  }));
  // The same list while the finds and type filter stay, so the table (a thousand rows) isn't drawn again.
  const shown = useMemo(
    () => (kinds.length > 0 ? finds.filter((find) => kinds.includes(find.kind)) : finds),
    [finds, kinds],
  );
  const chosen = useMemo(() => {
    const picked = new Set(selected);
    return finds.filter((find) => picked.has(find.url));
  }, [finds, selected]);

  async function runScan() {
    const url = link.trim();
    if (!url || scanning) return;
    setScanning(true);
    setError(null);
    try {
      await scan(url);
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
      await grab(chosen);
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
                disabled={!link.trim()}
                onClick={runScan}
              />
            }
          />
        </Field>
        {scanning && (
          <span className="grab-scanning">
            <Spinner />
            Scanning the page
          </span>
        )}
      </Panel>

      {page && !scanning && (
        <Panel as="section" className="grab-results" aria-label="Found on the page">
          <div className="grab-head">
            <span className="grab-title">{page.title ?? hostOf(page.url)}</span>
            <span className="grab-facts">
              {finds.length} found · {hostOf(page.url)}
            </span>
          </div>
          {finds.length === 0 ? (
            <EmptyState title="Nothing to download on this page" />
          ) : (
            <>
              <div className="grab-toolbar">
                <MultiSegmented
                  size="sm"
                  aria-label="Types"
                  allLabel="All"
                  options={options}
                  value={kinds}
                  onValueChange={setKinds}
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
              </div>
              <FindTable finds={shown} picked={picked} selected={selected} onSelectedChange={setSelected} />
            </>
          )}
        </Panel>
      )}
    </div>
  );
}

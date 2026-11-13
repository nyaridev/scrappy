// MirrorPicks: a mirror's images and videos to pick from (ADR-0033), as Grab shows its finds, growing while the mirror
// crawls. The picked ones are saved into the mirror, a video over Max video size skipped, and its pages then show
// the copies.

import { Button, EmptyState, Field, Icon, NumberField, Panel, Spinner, toast } from "iro-ui";
import { DownloadIcon } from "iro-ui/icons";
import { useMemo, useState } from "react";
import { useSettings } from "../../app/SettingsProvider";
import { FindResults } from "../../components/finds/FindResults";
import { useMirrorMedia } from "./useMirrorMedia";

// ── Constants ─────────────────────────────
const MAX_VIDEO_MB = { start: 200, max: 100_000 };

// ── Public API ────────────────────────────
/** `site`'s mirror media; `crawling` while its crawl runs. */
export function MirrorPicks({ site, crawling }: { site: string; crawling: boolean }) {
  const { media, busy, save } = useMirrorMedia(site, crawling);
  const [selected, setSelected] = useState<string[]>([]);
  const start = useSettings().settings?.preferences?.starts.scrape.maxVideoMb;
  const [maxVideoMb, setMaxVideoMb] = useState(start ?? MAX_VIDEO_MB.start);
  const [starting, setStarting] = useState(false);
  const [shownSite, setShownSite] = useState(site);

  // Another site's mirror starts with nothing picked.
  if (site !== shownSite) {
    setShownSite(site);
    setSelected([]);
  }

  const finds = media?.finds ?? [];
  const kept = media?.kept ?? {};
  const saved = Object.values(kept).filter((one) => one?.state === "saved").length;
  const chosen = useMemo(() => {
    const picked = new Set(selected);
    return finds.filter((find) => picked.has(find.url)).map((find) => find.url);
  }, [finds, selected]);

  async function download() {
    setStarting(true);
    try {
      await save(chosen, maxVideoMb);
      setSelected([]);
    } catch (e) {
      toast({ tone: "error", title: "Couldn't save the picks", description: String(e) });
    } finally {
      setStarting(false);
    }
  }

  return (
    <Panel as="section" className="finds-panel" aria-label="Images and videos">
      <div className="finds-head">
        <div className="finds-heading">
          <span className="finds-title">Images and videos</span>
          <span className="finds-facts" aria-live="polite">
            {busy && <Spinner />}
            {[
              crawling ? "Listing" : media?.busy ? "Saving" : null,
              `${finds.length} found`,
              saved > 0 ? `${saved} saved` : null,
            ]
              .filter(Boolean)
              .join(" · ")}
          </span>
        </div>
      </div>
      {finds.length === 0 ? (
        !busy && <EmptyState title="No images or videos on these pages" />
      ) : (
        <FindResults finds={finds} kept={kept} selected={selected} onSelectedChange={setSelected} fresh={site}>
          <Field label="Max video size (MB)" layout="left">
            <NumberField
              size="sm"
              value={maxVideoMb}
              min={1}
              max={MAX_VIDEO_MB.max}
              step={50}
              onValueChange={(value) => setMaxVideoMb(value ?? MAX_VIDEO_MB.start)}
            />
          </Field>
          <Button
            variant="primary"
            icon={<Icon icon={DownloadIcon} />}
            loading={starting}
            disabled={chosen.length === 0 || busy}
            onClick={download}
          >
            {chosen.length > 0 ? `Download ${chosen.length}` : "Download"}
          </Button>
        </FindResults>
      )}
    </Panel>
  );
}

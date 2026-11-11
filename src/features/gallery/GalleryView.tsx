// GalleryView: everything in the library folder as a grid of thumbnails, updated as the folder changes: search, filter
// by type and site, sort, and more rows as you scroll. A picture opens large in a lightbox; other files open in their app. "Find duplicates" swaps the grid
// for the duplicate finder.

import { Button, EmptyState, Icon, Lightbox, Select, Spinner, TextField, toast } from "iro-ui";
import { DuplicateIcon, ImagesIcon, SearchIcon } from "iro-ui/icons";
import { useEffect, useRef, useState } from "react";
import { DuplicatesPanel } from "../../components/DuplicatesPanel";
import { openFile, picture } from "../../lib/backend";
import { formatCount } from "../../lib/format";
import { kindOf } from "../../lib/kinds";
import type { GalleryItem } from "../../lib/types/GalleryItem";
import type { GallerySort } from "../../lib/types/GallerySort";
import { GalleryTile } from "./GalleryTile";
import { useGallery } from "./useGallery";

// ── Constants ─────────────────────────────
const ALL = "all";
const SORTS: { value: GallerySort; label: string }[] = [
  { value: "newest", label: "Newest" },
  { value: "oldest", label: "Oldest" },
  { value: "largest", label: "Largest" },
  { value: "name", label: "Name" },
];
/** How long typing pauses before the search runs. */
const SEARCH_DELAY_MS = 250;

// ── Public API ────────────────────────────
/** Gallery */
export function GalleryView() {
  const [typed, setTyped] = useState("");
  const [search, setSearch] = useState("");
  const [kind, setKind] = useState(ALL);
  const [site, setSite] = useState(ALL);
  const [sort, setSort] = useState<GallerySort>("newest");
  const [shown, setShown] = useState<{ src: string; alt: string } | null>(null);
  const [finding, setFinding] = useState(false);
  const end = useRef<HTMLDivElement>(null);
  const { items, total, kinds, sites, loading, error, loadMore, reload } = useGallery({
    kind: kind === ALL ? null : kind,
    site: site === ALL ? null : site,
    search,
    sort,
  });

  useEffect(() => {
    const timer = setTimeout(() => setSearch(typed), SEARCH_DELAY_MS);
    return () => clearTimeout(timer);
  }, [typed]);

  // Loads the next page when the end of the grid comes near.
  useEffect(() => {
    const target = end.current;
    if (!target) return;
    const observer = new IntersectionObserver((seen) => seen[0]?.isIntersecting && loadMore(), {
      rootMargin: "600px",
    });
    observer.observe(target);
    return () => observer.disconnect();
  }, [loadMore]);

  async function open(item: GalleryItem) {
    try {
      const src = await picture(item.path);
      if (src) setShown({ src, alt: item.name });
      else await openFile(item.path);
    } catch (e) {
      toast({ tone: "error", title: "Couldn't open the file", description: String(e) });
    }
  }

  const kindOptions = [
    { value: ALL, label: "All types" },
    ...kinds.map((name) => ({ value: name, label: kindOf(name).label })),
  ];
  const siteOptions = [{ value: ALL, label: "All sites" }, ...sites.map((name) => ({ value: name, label: name }))];
  const filtered = search !== "" || kind !== ALL || site !== ALL;

  if (finding) {
    return (
      <div className="gallery">
        <DuplicatesPanel
          onClose={(changed) => {
            setFinding(false);
            if (changed) reload();
          }}
        />
      </div>
    );
  }

  return (
    <div className="gallery">
      <div className="gallery-toolbar">
        <TextField
          className="gallery-search"
          aria-label="Search"
          placeholder="Search"
          value={typed}
          onChange={(event) => setTyped(event.target.value)}
          spellCheck={false}
          autoComplete="off"
          start={<Icon icon={SearchIcon} />}
        />
        <Select aria-label="Type" options={kindOptions} value={kind} onValueChange={(v) => setKind(v ?? ALL)} />
        <Select aria-label="Site" options={siteOptions} value={site} onValueChange={(v) => setSite(v ?? ALL)} />
        <Select aria-label="Sort" options={SORTS} value={sort} onValueChange={(v) => setSort(v ?? "newest")} />
        <Button icon={<Icon icon={DuplicateIcon} />} onClick={() => setFinding(true)}>
          Duplicates
        </Button>
      </div>
      {total != null && total > 0 && <span className="gallery-count">{formatCount(total, "file")}</span>}

      {error ? (
        <EmptyState tone="warning" title="Couldn't read the library">
          {error}
        </EmptyState>
      ) : total === 0 ? (
        <EmptyState
          icon={<Icon icon={ImagesIcon} size={32} />}
          title={filtered ? "Nothing matches" : "No downloads yet"}
        />
      ) : (
        <ul className="gallery-grid" aria-label="Library files">
          {items.map((item) => (
            <GalleryTile key={item.path} item={item} onOpen={(picked) => void open(picked)} />
          ))}
        </ul>
      )}
      <div ref={end} className="gallery-end">
        {loading && <Spinner label="Loading more files" />}
      </div>

      <Lightbox
        images={shown ? [{ src: shown.src, alt: shown.alt, caption: shown.alt }] : []}
        open={shown != null}
        onOpenChange={(open) => !open && setShown(null)}
      />
    </div>
  );
}

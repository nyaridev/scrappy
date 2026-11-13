// GalleryView: everything in the library folder as a grid of thumbnails, updated as the folder changes: a folder
// sidebar, a search that runs as you type, a type filter, sort, and more rows as you scroll. A picture, video or audio
// file opens in the viewer (ADR-0023); other files open in their app. "Find duplicates" swaps the grid for the duplicate finder. Delete (on a
// tile's or a downloaded folder's right-click) asks first, then deletes for good (ADR-0022).

import { AlertDialog, EmptyState, Icon, IconButton, SingleSelect, Spinner, TextField, toast } from "iro-ui";
import { DuplicateIcon, ImagesIcon, SearchIcon } from "iro-ui/icons";
import { useEffect, useRef, useState } from "react";
import { useViewActive } from "../../app/viewShown";
import { DuplicatesPanel } from "../../components/DuplicatesPanel";
import { deleteFromLibrary, openFile } from "../../lib/backend";
import { formatCount } from "../../lib/format";
import { kindOf } from "../../lib/kinds";
import type { GalleryItem } from "../../lib/types/GalleryItem";
import type { GallerySort } from "../../lib/types/GallerySort";
import { FolderTree } from "./FolderTree";
import { GalleryTile } from "./GalleryTile";
import { MediaViewer } from "./MediaViewer";
import { useGallery } from "./useGallery";
import "./gallery.css";

// ── Constants ─────────────────────────────
const ALL = "all";
const SORTS: { value: GallerySort; label: string }[] = [
  { value: "newest", label: "Newest" },
  { value: "oldest", label: "Oldest" },
  { value: "largest", label: "Largest" },
  { value: "name", label: "Name" },
];

// ── Public API ────────────────────────────
/** Gallery */
export function GalleryView() {
  const [search, setSearch] = useState("");
  const [kind, setKind] = useState(ALL);
  const [folder, setFolder] = useState<string | null>(null);
  const [sort, setSort] = useState<GallerySort>("newest");
  // The file the viewer shows, by path, so it stays on it as the rows reload.
  const [viewing, setViewing] = useState<string | null>(null);
  // The viewer leaves while another view shows (its video stops, its keys free) and comes back on the same file.
  const active = useViewActive();
  const [finding, setFinding] = useState(false);
  // What a confirmed Delete removes: a file, or a folder with everything in it.
  const [deleting, setDeleting] = useState<{ path: string; folder: boolean } | null>(null);
  const end = useRef<HTMLDivElement>(null);
  const { items, shownFilter, total, kinds, folders, loading, error, loadMore, reload } = useGallery({
    kind: kind === ALL ? null : kind,
    folder,
    search,
    sort,
  });

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

  async function remove(path: string) {
    try {
      await deleteFromLibrary(path);
      // A deleted folder that was picked (or held the one picked) leaves nothing to show.
      if (folder != null && (folder === path || folder.startsWith(`${path}/`))) setFolder(null);
      reload();
    } catch (e) {
      toast({ tone: "error", title: "Couldn't delete it", description: String(e) });
    }
    setDeleting(null);
  }

  async function open(item: GalleryItem) {
    if (item.view != null) return setViewing(item.path);
    try {
      await openFile(item.path);
    } catch (e) {
      toast({ tone: "error", title: "Couldn't open the file", description: String(e) });
    }
  }

  const kindOptions = [
    { value: ALL, label: "All types" },
    ...kinds.map((name) => ({ value: name, label: kindOf(name).label })),
  ];
  const filtered = search !== "" || kind !== ALL || folder != null;

  if (finding) {
    return (
      <div className="duplicates-page">
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
    <div className="gallery" data-viewing={(viewing && active) || undefined}>
      <FolderTree
        folders={folders}
        selected={folder}
        onSelect={setFolder}
        onDelete={(path) => setDeleting({ path, folder: true })}
      />
      <div className="gallery-main">
        <div className="gallery-toolbar">
          <TextField
            className="gallery-search"
            aria-label="Search"
            placeholder="Search"
            value={search}
            onChange={(event) => setSearch(event.target.value)}
            spellCheck={false}
            autoComplete="off"
            start={<Icon icon={SearchIcon} />}
          />
          <div className="gallery-choices">
            <SingleSelect
              aria-label="Type"
              options={kindOptions}
              value={kind}
              onValueChange={(v) => setKind(v ?? ALL)}
            />
            <SingleSelect
              aria-label="Sort"
              options={SORTS}
              value={sort}
              onValueChange={(v) => setSort(v ?? "newest")}
            />
            <IconButton icon={<Icon icon={DuplicateIcon} />} label="Find duplicates" onClick={() => setFinding(true)} />
          </div>
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
          // A new folder, type or sort draws the grid anew, so its tiles arrive as the first ones did; typing doesn't.
          <ul
            key={`${shownFilter?.folder}|${shownFilter?.kind}|${shownFilter?.sort}`}
            className="gallery-grid"
            aria-label="Library files"
          >
            {items.map((item, index) => (
              <GalleryTile
                key={item.path}
                item={item}
                index={index}
                onOpen={(picked) => void open(picked)}
                onDelete={(picked) => setDeleting({ path: picked.path, folder: false })}
              />
            ))}
          </ul>
        )}
        <div ref={end} className="gallery-end">
          {loading && <Spinner label="Loading more files" />}
        </div>
      </div>

      <AlertDialog
        open={deleting != null}
        onOpenChange={(open) => !open && setDeleting(null)}
        title={`Delete ${deleting?.folder ? "the folder " : ""}"${deleting?.path.split("/").pop() ?? ""}"?`}
        description={
          deleting?.folder
            ? "Everything in it is deleted from the disk for good, not moved to the trash."
            : "It's deleted from the disk for good, not moved to the trash."
        }
        confirmLabel="Delete"
        onConfirm={() => (deleting ? remove(deleting.path) : undefined)}
      />

      {viewing && active && (
        <MediaViewer
          items={items}
          path={viewing}
          onPathChange={setViewing}
          onClose={() => setViewing(null)}
          onNearEnd={loadMore}
        />
      )}
    </div>
  );
}

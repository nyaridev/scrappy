// FindResults: a list of finds to pick from, as Grab and a mirror's media show them (ADR-0033): type filters with
// counts, sort and layout, Select all with the view's own actions beside it, the finds as a grid or a list, and 50 a
// page. The view keeps what's picked; the filter and the page start over with each new list.

import { Checkbox, Icon, MultiSegmented, Pagination, Segmented, Select, sortRows } from "iro-ui";
import { GridIcon, ListIcon } from "iro-ui/icons";
import { type ReactNode, useMemo, useRef, useState } from "react";
import type { Find } from "../../lib/types/Find";
import type { FindKind } from "../../lib/types/FindKind";
import type { Kept } from "../../lib/types/Kept";
import type { Picked } from "../../lib/types/Picked";
import { TipLabel } from "../TipLabel";
import { FindGrid } from "./FindGrid";
import { FindList } from "./FindList";
import { FIND_KINDS } from "./findKinds";
import "./finds.css";

// ── Constants ─────────────────────────────
const KINDS = Object.keys(FIND_KINDS) as FindKind[];
type Layout = "list" | "grid";
const LAYOUTS = [
  { value: "grid" as const, label: "Grid", icon: <Icon icon={GridIcon} /> },
  { value: "list" as const, label: "List", icon: <Icon icon={ListIcon} /> },
];
const PAGE_SIZE = 50;
type Sort = "type" | "name" | "largest" | "smallest";
const SORTS: { value: Sort; label: string }[] = [
  { value: "type", label: "Type" },
  { value: "name", label: "Name" },
  { value: "largest", label: "Largest" },
  { value: "smallest", label: "Smallest" },
];
/** Each sort; finds of unknown size go last either way. Rust sends them by type, then name. */
const SORTED: Record<Sort, (finds: Find[]) => Find[]> = {
  type: (finds) => finds,
  name: (finds) => sortRows(finds, (find) => find.name.toLowerCase(), "asc"),
  largest: (finds) => sortRows(finds, (find) => find.size ?? -1, "desc"),
  smallest: (finds) => sortRows(finds, (find) => find.size ?? Number.MAX_SAFE_INTEGER, "asc"),
};
const NO_PICKS: Record<string, Picked> = {};

// ── Types ─────────────────────────────────
interface Props {
  finds: Find[];
  /** Grab's picked downloads, by link. */
  picked?: Record<string, Picked>;
  /** How saving each find went, by link. */
  kept: { [url in string]?: Kept };
  selected: string[];
  onSelectedChange: (urls: string[]) => void;
  /** Changes with each new list: the type filter and the page start over. */
  fresh: unknown;
  /** Beside Select all, at the right: the view's download button and its options. */
  children: ReactNode;
}

// ── Public API ────────────────────────────
/** The finds with their toolbars and pages. */
export function FindResults({ finds, picked = NO_PICKS, kept, selected, onSelectedChange, fresh, children }: Props) {
  const [kinds, setKinds] = useState<FindKind[]>([]);
  const [layout, setLayout] = useState<Layout>("grid");
  const [sort, setSort] = useState<Sort>("type");
  const [pageNumber, setPageNumber] = useState(1);
  const [shownFresh, setShownFresh] = useState(fresh);
  const top = useRef<HTMLDivElement>(null);

  if (fresh !== shownFresh) {
    setShownFresh(fresh);
    setKinds([]);
    setPageNumber(1);
  }

  const counts = Object.fromEntries(KINDS.map((kind) => [kind, finds.filter((f) => f.kind === kind).length]));
  const options = KINDS.filter((kind) => counts[kind] > 0).map((kind) => ({
    value: kind,
    label: (
      <TipLabel
        icon={FIND_KINDS[kind].icon}
        label={`${FIND_KINDS[kind].label} ${counts[kind]}`}
        tip={FIND_KINDS[kind].about}
      />
    ),
  }));
  // The same list while the finds, type filter and sort stay, so the page of rows isn't drawn again.
  const shown = useMemo(
    () => SORTED[sort](kinds.length > 0 ? finds.filter((find) => kinds.includes(find.kind)) : finds),
    [finds, kinds, sort],
  );
  const current = Math.min(pageNumber, Math.max(1, Math.ceil(shown.length / PAGE_SIZE)));
  const rows = useMemo(() => shown.slice((current - 1) * PAGE_SIZE, current * PAGE_SIZE), [shown, current]);
  const shownPicked = useMemo(() => {
    const chosen = new Set(selected);
    return shown.filter((find) => chosen.has(find.url)).length;
  }, [shown, selected]);

  function selectAll(all: boolean) {
    const urls = new Set(shown.map((find) => find.url));
    const rest = selected.filter((url) => !urls.has(url));
    onSelectedChange(all ? [...rest, ...urls] : rest);
  }

  function turnPage(next: number) {
    setPageNumber(next);
    top.current?.scrollIntoView({ block: "start" });
  }

  return (
    <>
      <div ref={top} className="finds-toolbar">
        <MultiSegmented
          size="sm"
          aria-label="Types"
          allLabel="All"
          options={options}
          value={kinds}
          onValueChange={(next) => {
            setKinds(next);
            setPageNumber(1);
          }}
        />
        <div className="finds-controls">
          <Select
            size="sm"
            aria-label="Sort"
            options={SORTS}
            value={sort}
            onValueChange={(next) => setSort(next ?? "type")}
          />
          <Segmented size="sm" aria-label="Layout" options={LAYOUTS} value={layout} onValueChange={setLayout} />
        </div>
      </div>
      <div className="finds-toolbar">
        <Checkbox
          label={`Select all ${shown.length}`}
          checked={shown.length > 0 && shownPicked === shown.length}
          indeterminate={shownPicked > 0 && shownPicked < shown.length}
          onCheckedChange={selectAll}
        />
        <div className="finds-controls finds-actions">{children}</div>
      </div>
      {layout === "list" ? (
        <FindList finds={rows} picked={picked} kept={kept} selected={selected} onSelectedChange={onSelectedChange} />
      ) : (
        <FindGrid finds={rows} picked={picked} kept={kept} selected={selected} onSelectedChange={onSelectedChange} />
      )}
      {shown.length > PAGE_SIZE && (
        <Pagination page={current} pageSize={PAGE_SIZE} total={shown.length} onPageChange={turnPage} />
      )}
    </>
  );
}

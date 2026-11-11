// videoFilters: the picker's search, sort, length and picked filters over a list Rust sent; display only.

import type { ListEntry } from "../../lib/types/ListEntry";

// ── Constants ─────────────────────────────
export const SORTS = [
  { value: "list", label: "List order" },
  { value: "reverse", label: "Reversed" },
  { value: "views", label: "Most viewed" },
  { value: "fewest", label: "Least viewed" },
  { value: "longest", label: "Longest" },
  { value: "shortest", label: "Shortest" },
  { value: "title", label: "Title A–Z" },
] as const;

/** YouTube's own length buckets. */
export const LENGTHS = [
  { value: "any", label: "Any length", min: 0, max: Number.POSITIVE_INFINITY },
  { value: "short", label: "Under 4 min", min: 0, max: 240 },
  { value: "medium", label: "4–20 min", min: 240, max: 1200 },
  { value: "long", label: "Over 20 min", min: 1200, max: Number.POSITIVE_INFINITY },
] as const;

export const SHOWS = [
  { value: "all", label: "All" },
  { value: "picked", label: "Picked" },
  { value: "dropped", label: "Not picked" },
] as const;

// ── Types ─────────────────────────────────
export type Sort = (typeof SORTS)[number]["value"];
export type Length = (typeof LENGTHS)[number]["value"];
export type Show = (typeof SHOWS)[number]["value"];

export interface VideoFilter {
  search: string;
  sort: Sort;
  length: Length;
  show: Show;
}

// ── Private helpers ───────────────────────
/** Missing numbers sort last whichever way the list is sorted. */
function byNumber(value: (entry: ListEntry) => number | null, descending: boolean) {
  return (a: ListEntry, b: ListEntry) => {
    const [x, y] = [value(a), value(b)];
    if (x == null || y == null) return x == null ? (y == null ? 0 : 1) : -1;
    return descending ? y - x : x - y;
  };
}

const COMPARE: Record<Exclude<Sort, "list" | "reverse">, (a: ListEntry, b: ListEntry) => number> = {
  views: byNumber((entry) => entry.views, true),
  fewest: byNumber((entry) => entry.views, false),
  longest: byNumber((entry) => entry.duration, true),
  shortest: byNumber((entry) => entry.duration, false),
  title: (a, b) => a.title.localeCompare(b.title, undefined, { numeric: true, sensitivity: "base" }),
};

// ── Public API ────────────────────────────
export const NO_FILTER: VideoFilter = { search: "", sort: "list", length: "any", show: "all" };

/** The entries the filter lets through, in its order; `off` holds the links turned off. */
export function shownVideos(entries: ListEntry[], filter: VideoFilter, off: Set<string>): ListEntry[] {
  const search = filter.search.trim().toLowerCase();
  const length = LENGTHS.find((each) => each.value === filter.length) ?? LENGTHS[0];
  const shown = entries.filter(
    (entry) =>
      (!search || entry.title.toLowerCase().includes(search)) &&
      (length.value === "any" ||
        (entry.duration != null && entry.duration >= length.min && entry.duration < length.max)) &&
      (filter.show === "all" || (filter.show === "picked") !== off.has(entry.url)),
  );
  if (filter.sort === "reverse") return shown.reverse();
  return filter.sort === "list" ? shown : shown.sort(COMPARE[filter.sort]);
}

/** Whether anything narrows the list (the sort doesn't). */
export function narrows(filter: VideoFilter): boolean {
  return filter.search.trim() !== "" || filter.length !== "any" || filter.show !== "all";
}

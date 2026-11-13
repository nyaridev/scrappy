// VideoPicker: a pop-up with every video of a link's lists as a grid of cards (a channel's Videos and Shorts behind
// a switch at the top), with a search, a sort and length and picked filters, 100 cards a page; clicking a card picks
// or drops it, and Select all / none act on every card the filters show. The dropped ones are left out of the
// download. A list still being listed grows in place, on the page shown (ADR-0030).

import { Button, Dialog, Icon, IconButton, Pagination, Segmented, Select, TextField } from "iro-ui";
import { CloseIcon, SearchIcon } from "iro-ui/icons";
import { type Dispatch, type SetStateAction, useRef, useState } from "react";
import type { Section } from "../../lib/types/Section";
import { VideoCard } from "./VideoCard";
import { LENGTHS, type Length, NO_FILTER, narrows, SHOWS, SORTS, type Sort, shownVideos } from "./videoFilters";

// ── Constants ─────────────────────────────
const PAGE_SIZE = 100;

// ── Public API ────────────────────────────
/** The pop-up, opened on `first`; `skip` holds the links turned off, across every list. */
export function VideoPicker({
  lists,
  first,
  skip,
  onSkipChange,
  onClose,
}: {
  lists: Section[];
  first: string;
  skip: string[];
  onSkipChange: Dispatch<SetStateAction<string[]>>;
  onClose: () => void;
}) {
  const [current, setCurrent] = useState(first);
  const [filter, setFilter] = useState(NO_FILTER);
  const [page, setPage] = useState(1);
  const grid = useRef<HTMLUListElement>(null);
  const list = lists.find((each) => each.url === current) ?? lists[0];
  const off = new Set(skip);
  const picked = list.entries.filter((entry) => !off.has(entry.url)).length;
  const shown = shownVideos(list.entries, filter, off);
  const shownLinks = new Set(shown.map((entry) => entry.url));
  const shownPicked = shown.filter((entry) => !off.has(entry.url)).length;
  const shownPage = Math.min(page, Math.max(1, Math.ceil(shown.length / PAGE_SIZE)));
  const cards = shown.slice((shownPage - 1) * PAGE_SIZE, shownPage * PAGE_SIZE);
  const hasLengths = list.entries.some((entry) => entry.duration != null);
  const hasViews = list.entries.some((entry) => entry.views != null);
  const sorts = SORTS.filter((sort) => {
    if (sort.value === "views" || sort.value === "fewest") return hasViews;
    if (sort.value === "longest" || sort.value === "shortest") return hasLengths;
    return true;
  });
  const toggle = (url: string) =>
    onSkipChange((all) => (all.includes(url) ? all.filter((each) => each !== url) : [...all, url]));
  const change = (part: Partial<typeof filter>) => {
    setFilter((all) => ({ ...all, ...part }));
    setPage(1);
  };
  const turnPage = (next: number) => {
    setPage(next);
    grid.current?.scrollIntoView({ block: "start" });
  };

  return (
    <Dialog
      open
      onOpenChange={(open) => !open && onClose()}
      size="lg"
      className="picker"
      title={list.title}
      description={`${picked} of ${list.entries.length} picked`}
      footer={
        <Button variant="primary" onClick={onClose}>
          Done
        </Button>
      }
    >
      <div className="picker-bar">
        {lists.length > 1 && (
          <Segmented
            size="sm"
            aria-label="List"
            options={lists.map((each) => ({ value: each.url, label: each.title }))}
            value={list.url}
            onValueChange={(url) => {
              setCurrent(url);
              setPage(1);
              // A length filter or sort means nothing for a list without lengths (Shorts).
              setFilter((all) => ({ ...all, length: "any", sort: "list" }));
            }}
          />
        )}
        <TextField
          size="sm"
          className="picker-search"
          aria-label="Search titles"
          placeholder="Search titles"
          value={filter.search}
          onChange={(event) => change({ search: event.target.value })}
          start={<Icon icon={SearchIcon} />}
          end={
            filter.search && (
              <IconButton
                size="sm"
                icon={<Icon icon={CloseIcon} />}
                label="Clear"
                onClick={() => change({ search: "" })}
              />
            )
          }
        />
      </div>
      <div className="picker-bar">
        <span className="picker-filters">
          <Select
            size="sm"
            className="picker-select"
            aria-label="Sort"
            options={[...sorts]}
            value={filter.sort}
            onValueChange={(sort) => change({ sort: (sort ?? "list") as Sort })}
          />
          {hasLengths && (
            <Select
              size="sm"
              className="picker-select"
              aria-label="Length"
              options={LENGTHS.map(({ value, label }) => ({ value, label }))}
              value={filter.length}
              onValueChange={(length) => change({ length: (length ?? "any") as Length })}
            />
          )}
          <Segmented
            size="sm"
            aria-label="Show"
            options={[...SHOWS]}
            value={filter.show}
            onValueChange={(show) => change({ show })}
          />
        </span>
        <span className="picker-actions">
          {narrows(filter) && <span className="picker-shown">{shown.length} shown</span>}
          <Button
            size="sm"
            variant="ghost"
            disabled={shownPicked === shown.length}
            onClick={() => onSkipChange((all) => all.filter((url) => !shownLinks.has(url)))}
          >
            Select all
          </Button>
          <Button
            size="sm"
            variant="ghost"
            disabled={shownPicked === 0}
            onClick={() => onSkipChange((all) => [...new Set([...all, ...shownLinks])])}
          >
            Select none
          </Button>
        </span>
      </div>
      {shown.length === 0 ? (
        <p className="picker-empty">Nothing matches.</p>
      ) : (
        <>
          <ul ref={grid} className="video-grid" data-tall={list.entries[0]?.tall || undefined} aria-label={list.title}>
            {cards.map((entry) => (
              <VideoCard
                key={entry.url}
                entry={entry}
                picked={!off.has(entry.url)}
                onToggle={() => toggle(entry.url)}
              />
            ))}
          </ul>
          {shown.length > PAGE_SIZE && (
            <Pagination page={shownPage} pageSize={PAGE_SIZE} total={shown.length} onPageChange={turnPage} />
          )}
        </>
      )}
    </Dialog>
  );
}

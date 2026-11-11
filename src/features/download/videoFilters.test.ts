import { describe, expect, it } from "vitest";
import type { ListEntry } from "../../lib/types/ListEntry";
import { NO_FILTER, shownVideos } from "./videoFilters";

const entry = (title: string, duration: number | null, views: number | null): ListEntry => ({
  url: `https://youtu.be/${title}`,
  title,
  duration,
  views,
  tall: false,
});
const LIST = [entry("b remix", 100, 10), entry("a song", 1500, null), entry("c live", null, 900)];
const titles = (entries: ListEntry[]) => entries.map((each) => each.title);

describe("shownVideos", () => {
  it("sorts with missing numbers last", () => {
    expect(titles(shownVideos(LIST, { ...NO_FILTER, sort: "views" }, new Set()))).toEqual([
      "c live",
      "b remix",
      "a song",
    ]);
    expect(titles(shownVideos(LIST, { ...NO_FILTER, sort: "shortest" }, new Set()))).toEqual([
      "b remix",
      "a song",
      "c live",
    ]);
    expect(titles(shownVideos(LIST, { ...NO_FILTER, sort: "reverse" }, new Set()))).toEqual([
      "c live",
      "a song",
      "b remix",
    ]);
  });

  it("filters by search, length and picked", () => {
    expect(titles(shownVideos(LIST, { ...NO_FILTER, search: " REMIX" }, new Set()))).toEqual(["b remix"]);
    expect(titles(shownVideos(LIST, { ...NO_FILTER, length: "long" }, new Set()))).toEqual(["a song"]);
    const off = new Set(["https://youtu.be/a song"]);
    expect(titles(shownVideos(LIST, { ...NO_FILTER, show: "dropped" }, off))).toEqual(["a song"]);
    expect(titles(shownVideos(LIST, { ...NO_FILTER, show: "picked" }, off))).toEqual(["b remix", "c live"]);
  });
});

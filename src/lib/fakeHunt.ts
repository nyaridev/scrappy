// fakeHunt: the preview's hunt (`pnpm dev`), for `backend.ts`. Its sources go through their states the way Rust's do
// (ADR-0024 to ADR-0026): image search engines first, four sources at once, one search at a time per host, results
// gone through up to each source's limit. The first site's search fails. With "Remove duplicates" on, it ends by
// removing about a sixth of what it saved (ADR-0027).

import type { HuntRequest } from "./types/HuntRequest";
import type { HuntSource } from "./types/HuntSource";
import type { HuntStatus } from "./types/HuntStatus";
import type { ImageEngine } from "./types/ImageEngine";

// ── Constants ─────────────────────────────
const AT_ONCE = 4;
/** Ticks one search takes. */
const SEARCH_TICKS = 4;
/** How long removing duplicates takes. */
const REMOVING_MS = 1500;
const TICK_MS = 150;
const ENGINES: [ImageEngine, string][] = [
  ["duckDuckGo", "DuckDuckGo Images"],
  ["bing", "Bing Images"],
  ["yandex", "Yandex Images"],
];

// ── Types ─────────────────────────────────
interface Calls {
  /** Images saved of the total, and the job's message. */
  report: (done: number, total: number, message: string) => void;
  stopped: () => boolean;
  /** The hunt ended: stopped, or done with `saved` images. */
  end: (stopped: boolean, saved: number) => void;
}

// ── Public API ────────────────────────────
/** Runs a fake hunt; returns a reader of the hunt as it is now. */
export function fakeHunt(request: HuntRequest, calls: Calls): () => HuntStatus {
  let removing = false;
  let removed: number | null = null;
  const engineSources = new Set<HuntSource>();
  const source = (name: string, engine: boolean): HuntSource => {
    const one: HuntSource = {
      name,
      state: "waiting",
      read: 0,
      found: 0,
      saved: 0,
      limit: request.perSource,
      problem: null,
    };
    if (engine) engineSources.add(one);
    return one;
  };
  const isEngine = (one: HuntSource) => engineSources.has(one);
  const engines = ENGINES.filter(([engine]) => request.engines.includes(engine)).map(([, name]) => source(name, true));
  const sites = [...(request.web ? ["Any site"] : []), ...request.sites].map((name) => source(name, false));
  const sources = [...engines, ...sites];
  const failing = sites[0];
  // Searches on one host take turns: Bing and Yandex have their own, the rest are DuckDuckGo's.
  const host = (one: HuntSource) => (isEngine(one) && one.name !== "DuckDuckGo Images" ? one.name : "DuckDuckGo");
  const ticks = new Map<HuntSource, number>();
  const ended = (one: HuntSource) => one.state === "done" || one.state === "failed" || one.state === "stopped";
  const total = request.perSource * sources.length;

  const timer = setInterval(() => {
    const stopped = calls.stopped();
    if (stopped) for (const one of sources) if (!ended(one)) one.state = "stopped";
    // The first four that haven't ended are the ones running.
    const running = sources.filter((one) => !ended(one)).slice(0, AT_ONCE);
    for (const one of running) {
      if (one.state === "reading") {
        one.read = Math.min(one.found, one.read + (isEngine(one) ? 6 : 1));
        one.saved = Math.min(one.limit, one.saved + Math.floor(Math.random() * (isEngine(one) ? 6 : 4)));
        if (one.read >= one.found || one.saved >= one.limit) one.state = "done";
      } else if (one.state === "searching") {
        const tick = (ticks.get(one) ?? 0) + 1;
        ticks.set(one, tick);
        if (tick < SEARCH_TICKS) continue;
        if (one === failing) {
          one.state = "failed";
          one.problem = "Bing and DuckDuckGo didn't answer. Try again in a while.";
        } else {
          one.state = "reading";
          one.found = one.name === "Yandex Images" ? 30 : isEngine(one) ? one.limit * 2 : one.limit + request.pages;
        }
      } else if (!stopped && !running.some((other) => other.state === "searching" && host(other) === host(one))) {
        one.state = "searching";
      }
    }

    const saved = sources.reduce((sum, one) => sum + one.saved, 0);
    const done = sources.filter((one) => one.state === "done").length;
    calls.report(saved, total, `${done} of ${sources.length} sources done`);
    if (sources.filter(ended).length < sources.length) return;
    clearInterval(timer);
    if (stopped || !request.removeDuplicates || saved < 2) return calls.end(stopped, saved);
    removing = true;
    setTimeout(() => {
      removing = false;
      removed = Math.floor(saved / 6);
      calls.end(false, saved - removed);
    }, REMOVING_MS);
  }, TICK_MS);

  return () => ({ sources: sources.map((one) => ({ ...one })), removing, removed });
}

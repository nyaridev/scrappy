// useCached: a value from Rust kept for the whole session, so a view opened again shows it in its first frame (no
// empty rows popping in, no layout shift) while a fresh copy loads, each time its view is shown.

import { useCallback, useEffect, useState } from "react";
import { useViewShown } from "./viewShown";

// ── State ─────────────────────────────────
const values = new Map<string, unknown>();

// ── Public API ────────────────────────────
/** The value for `key` (`null` until the first load ever finishes) and a setter that updates the cache too. `load`
 * must be stable: pass a `backend.ts` function. */
export function useCached<T>(key: string, load: () => Promise<T>): [T | null, (value: T) => void] {
  const [value, setValue] = useState<T | null>(() => (values.has(key) ? (values.get(key) as T) : null));
  const update = useCallback(
    (fresh: T) => {
      values.set(key, fresh);
      setValue(fresh);
    },
    [key],
  );
  const shown = useViewShown();
  // biome-ignore lint/correctness/useExhaustiveDependencies: `shown` is there to load again when the view shows
  useEffect(() => void load().then(update), [load, update, shown]);
  return [value, update];
}

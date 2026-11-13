// usePick: picking finds with a click; a shift-click picks every find between it and the one clicked before.

import { useRef } from "react";
import type { Find } from "../../lib/types/Find";

/** The click handler for the finds shown: `range` is a shift-click. */
export function usePick(finds: Find[], selected: string[], onSelectedChange: (urls: string[]) => void) {
  const anchor = useRef<string | null>(null);
  return (url: string, range: boolean) => {
    const chosen = new Set(selected);
    const from = finds.findIndex((find) => find.url === anchor.current);
    const to = finds.findIndex((find) => find.url === url);
    anchor.current = url;
    if (range && from >= 0 && to >= 0) {
      for (const find of finds.slice(Math.min(from, to), Math.max(from, to) + 1)) chosen.add(find.url);
    } else if (!chosen.delete(url)) {
      chosen.add(url);
    }
    onSelectedChange([...chosen]);
  };
}

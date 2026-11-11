// KeptView: one opened view, kept mounted and hidden while another shows (App, `styles.css`). It's drawn again only
// when it shows or hides, not on every switch between the other views, and it keeps its scroll: hiding drops it, so
// it's noted as the view hides and put back when it shows.

import { type ComponentType, memo, useLayoutEffect, useMemo, useRef } from "react";
import { ViewActive, ViewShown } from "./viewShown";

/** The view's page, with whether it shows and how many times it has. */
export const KeptView = memo(function KeptView({
  label,
  active,
  shown,
  Page,
}: {
  label: string;
  active: boolean;
  shown: number;
  Page: ComponentType;
}) {
  // The same element every time, so showing or hiding redraws only the parts that read `ViewActive` or `ViewShown`.
  const page = useMemo(() => <Page />, [Page]);
  const main = useRef<HTMLElement>(null);
  const scroll = useRef(0);
  const wasActive = useRef(active);

  // Read while the old layout still stands: this render is the one that hides the view, before it's applied.
  if (wasActive.current && !active && main.current) scroll.current = main.current.scrollTop;
  wasActive.current = active;

  useLayoutEffect(() => {
    if (active && main.current) main.current.scrollTop = scroll.current;
  }, [active]);

  return (
    <main ref={main} className="view" aria-label={label} hidden={!active}>
      <ViewActive.Provider value={active}>
        <ViewShown.Provider value={shown}>{page}</ViewShown.Provider>
      </ViewActive.Provider>
    </main>
  );
});

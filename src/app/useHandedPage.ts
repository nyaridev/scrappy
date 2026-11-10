// useHandedPage: takes the page the extension handed to Grab, when the window starts and whenever it gains focus (the
// app focuses the window when a page arrives), shows it in Grab and calls `onHanded`.

import { useEffect } from "react";
import { showPage } from "../features/grab/useGrab";
import { handedPage, onWindowFocus } from "../lib/backend";

/** Watches for a handed page. */
export function useHandedPage(onHanded: () => void) {
  useEffect(() => {
    const take = () =>
      void handedPage().then((page) => {
        if (!page) return;
        showPage(page);
        onHanded();
      });
    take();
    return onWindowFocus(take);
  }, [onHanded]);
}

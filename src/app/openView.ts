// openView: lets a view open another one, as the Download view's "All downloads" opens the Downloads view.

import { createContext, useContext } from "react";

/** The Downloads view's id (`views.ts`). */
export const DOWNLOADS = "downloads";
/** Settings' id (`views.ts`). */
export const SETTINGS = "settings";

/** Opens the view with an id; `App` provides it. */
export const OpenView = createContext<(id: string) => void>(() => {});

/** The function that opens a view by its id. */
export function useOpenView(): (id: string) => void {
  return useContext(OpenView);
}

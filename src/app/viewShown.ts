// viewShown: how many times the view around a component has been shown, and whether it shows now. Views stay mounted
// when the header switches away (so their fields, results and scroll stay): a load that ran on mount runs again when
// the count changes, and work a hidden view doesn't need waits until it shows.

import { createContext, useContext } from "react";

// ── Public API ────────────────────────────
/** Set by `App` for each view it keeps. */
export const ViewShown = createContext(1);
export const ViewActive = createContext(true);

/** The count for the view this component is in; put it in a load's effect dependencies. */
export function useViewShown(): number {
  return useContext(ViewShown);
}

/** Whether the view this component is in shows now. */
export function useViewActive(): boolean {
  return useContext(ViewActive);
}

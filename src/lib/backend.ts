// Backend: the only file that talks to Rust (invoke and listen). Outside the app (`pnpm dev` in a browser) it
// answers with fake data, so every view can be previewed.

import { invoke } from "@tauri-apps/api/core";
import type { AppInfo } from "./types/AppInfo";

// ── Fake backend ──────────────────────────
const inTauri = "__TAURI_INTERNALS__" in window;

// ── App ───────────────────────────────────
/** The running version and platform. */
export async function appInfo(): Promise<AppInfo> {
  return inTauri ? invoke("app_info") : { version: "0.1.0", os: "browser preview" };
}

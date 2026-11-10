// Boundary check (ADR-0001): only src/lib/backend.ts may import Tauri in the window, and the extension never does.
// The Rust side needs no script: scrappy-core has no Tauri dependency, so Cargo refuses any import of it.

import { readdirSync, readFileSync } from "node:fs";
import { extname, join } from "node:path";

// ── Constants ─────────────────────────────
const ALLOWED = new Set([join("src", "lib", "backend.ts")]);
const TAURI_IMPORT = /from\s+["']@tauri-apps\//;
const ROOTS = ["src", "extension/entrypoints", "extension/lib"];

// ── Check ─────────────────────────────────
let failed = false;
for (const root of ROOTS) {
  let files;
  try {
    files = readdirSync(root, { recursive: true });
  } catch {
    continue;
  }
  for (const file of files) {
    const path = join(root, file);
    if (![".ts", ".tsx"].includes(extname(path)) || ALLOWED.has(path)) continue;
    if (TAURI_IMPORT.test(readFileSync(path, "utf8"))) {
      console.error(`boundary  ${path} imports @tauri-apps: call Rust through src/lib/backend.ts instead`);
      failed = true;
    }
  }
}
process.exit(failed ? 1 : 0);

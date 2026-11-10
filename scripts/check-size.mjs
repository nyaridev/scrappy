// Size check: fails when a source file is over 1200 lines and warns over 1000 (code-organization rule).
// Usage: node scripts/check-size.mjs [folder...]   (default: every source folder)

import { existsSync, readdirSync, readFileSync } from "node:fs";
import { extname, join } from "node:path";

// ── Constants ─────────────────────────────
const WARN = 1000;
const FAIL = 1200;
const EXTENSIONS = new Set([".rs", ".ts", ".tsx", ".css", ".mjs"]);
const ROOTS = ["crates", "src-tauri/src", "src", "extension/entrypoints", "extension/lib", "scripts"];
// Generated or built files: exempt.
const SKIP = /(^|[/])(target|node_modules|types)([/]|$)/;

// ── Check ─────────────────────────────────
const roots = process.argv.length > 2 ? process.argv.slice(2) : ROOTS;
let failed = false;
for (const root of roots.filter((r) => existsSync(r))) {
  for (const file of readdirSync(root, { recursive: true })) {
    if (!EXTENSIONS.has(extname(file)) || SKIP.test(file)) continue;
    const path = join(root, file);
    const lines = readFileSync(path, "utf8").trimEnd().split("\n").length;
    if (lines > FAIL) {
      console.error(`too long  ${path}: ${lines} lines (max ${FAIL}), split it`);
      failed = true;
    } else if (lines > WARN) {
      console.warn(`long      ${path}: ${lines} lines (over ${WARN})`);
    }
  }
}
process.exit(failed ? 1 : 0);

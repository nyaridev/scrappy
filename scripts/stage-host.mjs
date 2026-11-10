// Builds scrappy-host for the target a release is built for and puts it where Tauri's sidecar list expects it:
// src-tauri/binaries/scrappy-host-<target triple>[.exe] (ADR-0014). Release builds run it from
// src-tauri/tauri.release.conf.json; Tauri gives the target in TAURI_ENV_TARGET_TRIPLE. A universal macOS build gets
// both architectures and one file joining them.

import { execFileSync } from "node:child_process";
import { copyFileSync, mkdirSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

// ── Constants ─────────────────────────────
const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const OUT = join(ROOT, "src-tauri", "binaries");
const UNIVERSAL = "universal-apple-darwin";
const MAC_TARGETS = ["aarch64-apple-darwin", "x86_64-apple-darwin"];

// ── Private helpers ───────────────────────
function run(command, args) {
  execFileSync(command, args, { cwd: ROOT, stdio: "inherit" });
}

/** The machine's own target, when Tauri doesn't name one. */
function hostTriple() {
  const info = execFileSync("rustc", ["-vV"], { encoding: "utf8" });
  return info.match(/^host: (\S+)$/m)[1];
}

/** Builds the host for `triple`; returns the built file. */
function build(triple) {
  run("cargo", ["build", "--release", "-p", "scrappy-host", "--target", triple]);
  const exe = triple.includes("windows") ? "scrappy-host.exe" : "scrappy-host";
  return join(ROOT, "target", triple, "release", exe);
}

function staged(triple) {
  return join(OUT, `scrappy-host-${triple}${triple.includes("windows") ? ".exe" : ""}`);
}

// ── Main ──────────────────────────────────
const triple = process.env.TAURI_ENV_TARGET_TRIPLE || hostTriple();
mkdirSync(OUT, { recursive: true });
if (triple === UNIVERSAL) {
  for (const target of MAC_TARGETS) copyFileSync(build(target), staged(target));
  run("lipo", ["-create", "-output", staged(UNIVERSAL), ...MAC_TARGETS.map(staged)]);
} else {
  copyFileSync(build(triple), staged(triple));
}
console.log(`Staged scrappy-host for ${triple}`);

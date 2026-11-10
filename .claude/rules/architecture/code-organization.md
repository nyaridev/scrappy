---
paths:
  - "crates/**"
  - "src-tauri/src/**"
  - "src/**"
  - "extension/entrypoints/**"
  - "extension/lib/**"
  - "scripts/**"
---

# Code organization

## File size

- Up to 1000 lines: fine. 1001–1200: split when there's a clear seam, or say why you kept it whole. Over 1200: always
  split, along responsibilities, updating every import in the same step.
- `pnpm check` runs `scripts/check-size.mjs`: it fails over 1200 lines and warns over 1000.
- Before adding to a file over 1000 lines, split it first or put the new code in its own file.
- Exempt: generated files (`src/lib/types/`), and pure data tables (built-in presets, packs), which the rule names.

## One job per file

- Each file does one thing and its name says what. `lib.rs`, `main.rs`, `main.tsx` and `mod.rs` only wire things up.
- One React component per file, named after it (`FormatPicker.tsx`).
- A core module starts as `<name>.rs`; when it grows a second file, it becomes `<name>/mod.rs` plus one file per part.

## Section order

1. Header: 1–3 lines on what the file is for (`//!` in Rust, `//` in TypeScript).
2. Imports.
3. Constants.
4. Types.
5. State (module-level; rare).
6. Private helpers.
7. Public API, each item with a one-line doc comment.
8. Setup and exports. Rust ends with `#[cfg(test)] mod tests`.

Skip sections a file doesn't need, and never leave an empty divider. Files under about 40 lines may skip dividers. A
placeholder module is only its header comment.

## Dividers

```rust
//! Library: the download folder, its layout and the catalog.

// ── Imports ───────────────────────────────
use std::path::{Path, PathBuf};

// ── Constants ─────────────────────────────
const CATALOG: &str = ".scrappy/library.db";

// ── Types ─────────────────────────────────
pub struct Library {
    pub root: PathBuf,
}

// ── Public API ────────────────────────────
/// Opens the library at `root`, creating the catalog when it's missing.
pub fn open(root: &Path) -> Result<Library, String> {
    // ...
}

#[cfg(test)]
mod tests {
    // ...
}
```

TypeScript and CSS use the same `── Name ───` dividers (`// ──` and `/* ── */`).

## Comments and naming

- Comment why, not what, in one short line. No commented-out code, no stray `console.log`, `println!` or `dbg!`.
- Rust: `snake_case` files and functions. TypeScript: `camelCase.ts` modules, `PascalCase.tsx` components.
- The same thing has the same name in Rust and TypeScript (`Job`, `Preset`, `root`, `kind`); serde's camelCase does the
  rest.

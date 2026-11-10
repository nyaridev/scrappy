# Architecture

Scrappy is a desktop app for downloading videos and media from any site, grabbing what a page streams, crawling and
scraping images, and browsing it all. All logic lives in the Rust core crate; Tauri is a thin shell around it, and the
React + iro-ui window only shows what Rust sends.

## Shape

```
crates/scrappy-core/   all logic, one module per system; never uses Tauri
crates/scrappy-host/   native-messaging relay the browser starts (M5)
src-tauri/             Tauri 2 shell: lib.rs (setup), window.rs, events.rs, commands/<area>.rs, capabilities/
src/                   React 19 + iro-ui: app/ (header and title bar, views), features/<view>/, lib/backend.ts, lib/types/ (generated)
extension/             WXT browser extension: one popup, Chrome/Brave/Edge and Firefox builds
packs/                 built-in source packs for Hunt (lists of sites), TOML
docs/adr/              this overview and the decisions
scripts/               check-size.mjs, check-boundaries.mjs
```

## Modules

| Module | Owns | May depend on |
|---|---|---|
| `scrappy-core` | every behavior: library, jobs, tools, media, grab, crawl, hunt, dedupe, vault, bridge, settings, logging | crates.io crates; never `tauri` |
| `src-tauri` | the window, commands, capabilities, forwarding progress as events | `scrappy-core`, `tauri` |
| `scrappy-host` | relaying extension frames to the app | std, a local-socket crate; not `scrappy-core` |
| `src/` | showing state, passing clicks on | `iro-ui`, React; Rust only through `src/lib/backend.ts` |
| `extension/` | listing a page's media, sending a pick | WXT, React; the app only through native messaging |

Inside core, modules use each other only through their `pub` API. `jobs`, `logging`, `settings` and `library` are
the base; `tools` → `media` → `grab`/`crawl`/`hunt` build on them; `vault` and `bridge` are used by the rest.

## Decisions

| ADR | Title | Status |
|---|---|---|
| [0001](0001-rust-core-thin-shell-display-only-ui.md) | Rust core, thin Tauri shell, display-only UI | Accepted |
| [0002](0002-external-engines-as-managed-tools.md) | External engines as managed tools | Accepted |
| [0003](0003-editor-safe-encoding-presets.md) | Editor-safe encoding presets | Accepted |
| [0004](0004-library-folder-and-catalog.md) | The library folder and its catalog | Accepted |
| [0005](0005-security-and-secrets.md) | Security and secrets | Accepted |
| [0006](0006-browser-extension-quick-send.md) | Browser extension: a quick-send popup over native messaging | Accepted (host registration: 0014) |
| [0007](0007-grab-page-media-discovery.md) | Grab: finding everything downloadable on a page | Accepted |
| [0008](0008-crawl-and-offline-mirror.md) | Crawl and offline mirror | Accepted |
| [0009](0009-hunt-source-packs-and-dedupe.md) | Hunt: source packs, query detection and duplicate removal | Partly superseded by 0013 |
| [0010](0010-typed-commands-jobs-and-events.md) | Typed commands, jobs and events | Accepted |
| [0011](0011-frameless-window.md) | Frameless window | Accepted |
| [0012](0012-tool-sources-and-pinning.md) | Tool sources and pinning | Accepted |
| [0013](0013-hunt-web-search-and-content-images.md) | Hunt: web search and one content-image finder | Accepted |
| [0014](0014-builds-installers-and-host-registration.md) | Builds, installers and connecting the browsers | Accepted |

## Milestones

| Milestone | Goal | Status |
|---|---|---|
| M0 | Scaffold | done |
| M1 | Library and first-run setup | done |
| M2 | Tool manager (yt-dlp, Deno, gallery-dl, ffmpeg) | done |
| M3 | Download view: YouTube formats, presets, queue | done |
| M4 | Home: stats and recent downloads | done |
| M5 | Extension popup and host bridge | done |
| M6 | Cookie vault | done |
| M7 | Social and bulk downloads (X, Instagram, Facebook) | done |
| M8 | Grab view | done |
| M9 | Gallery | done |
| M10 | Duplicate removal | done |
| M11 | Image crawl with limits | done |
| M12 | Site mirror | done |
| M13 | Hunt: web search, content images, source packs | done |
| M14 | Windows, macOS and Linux builds in CI, installers | in progress |
| M15 | Change the library folder in Settings (another library, or move this one) | planned |

### M14: Windows, macOS and Linux builds in CI, installers

- [x] 0. ADR-0014 (supersedes ADR-0006's installer registration).
- [x] 1. A GitHub Actions check on every push and pull request: `pnpm check` on Windows, macOS and Linux (with
  WebKitGTK and the Secret Service libraries on Linux).
- [x] 2. A release workflow on a version tag: Tauri builds a Windows installer and the portable `Scrappy.exe`, a
  macOS disk image (Apple Silicon and Intel), and a Linux AppImage and `.deb`, into a draft GitHub release.
- [x] 3. `scrappy-host` built and shipped with each app, and a Settings button that registers it with the
  installed browsers for the installed app (today `pnpm register-host` does it for debug builds).
- [x] 4. The extension's Chrome and Firefox zips attached to the same release.
- [ ] 5. Done checks: each workflow runs green once on GitHub.

### Later

- The extension is tested by its parts (socket, relay, message checks) and a hello through the real host; load it
  in Chrome and Firefox (`pnpm register-host`, then load `extension/.output/`) to try a real send.
- Pick an app icon; `src-tauri/icons/` still holds the generator's, and the installers use them.
- Decide Scrappy's own licence before the repo goes public (the external tools are GPL; ADR-0002).
- macOS: the overlay title bar (header padding, traffic lights) is untested until there's a Mac build (M14).
- iro-ui 1.0.1: `TextField` lacks `box-sizing: border-box` (its slot-button math assumes it). Fix it there, then
  drop the override in `src/styles.css`.
- Tool pins age: bump them now and then (`core.md` → How to update a tool's pin), then run the ignored install test.
- Linux on ARM and Windows on ARM have no tool downloads yet; they use copies on `PATH`.
- GPU encoders (NVENC, QSV, AMF, VideoToolbox) for the editor-safe re-encode (ADR-0003), after M3's `libx264`.
- Downloads: a Retry button for failed ones; YouTube sometimes answers 403 mid-download, which a retry fixes.
- The Download list lasts one session; the catalog keeps the history (Home, M4).
- Images sent from the extension don't use remembered sign-ins yet (M6).
- Linux builds need the Secret Service (`libdbus`) for the vault's key; the workflows install it.
- Releases are unsigned: Windows SmartScreen and macOS Gatekeeper warn on first start (macOS: right-click → Open).
  Signing and notarizing need certificates (ADR-0014, Later).
- The workflows are untested until the repo is on GitHub (M14 step 5).
- A real sign-in flow (send → ask → share → download) is tested by its parts; try it once with a signed-in site
  in Chrome and Firefox.
- Collections are tested for real on a YouTube playlist and a Bluesky profile (the ignored test in
  `media/downloads.rs`). X, Instagram and Facebook need a sign-in: try each once through the extension.
- A playlist skips a video that fails and goes on; the finished row doesn't say how many failed yet.
- A send from the extension saves at most the first 50 files of a playlist or profile; the window picks any amount.
- Grab: `vsd` (ADR-0007's stream fallback) isn't added; add it when a stream turns up that yt-dlp can't read.
- Gallery thumbnails are tested with FFmpeg's test pictures; check them once on a real library (HEIC and AVIF depend
  on the FFmpeg build). `.scrappy/thumbs/` keeps a thumbnail after its file is gone.
- Duplicate removal: moving to the system trash is tested by its checks only (a real run would fill the trash of
  the PC running the tests); try it once in the app. It reads JPEG, PNG, WebP, GIF, BMP and TIFF; AVIF and HEIC
  are skipped. A picture edited in place keeps its old hash until it's downloaded again. Every pair is compared,
  which is fine to tens of thousands of pictures.
- Crawl: plain HTTP only; Chrome mode for script-heavy sites waits until a site needs it (ADR-0008). Links are
  sorted by their extension, so a CDN image link without one is missed. The Scrape view shows the folder; opening
  the Gallery filtered to it isn't built. The file crawl is tested on a local test site only.
- Mirror: a page's content that scripts build after it loads isn't saved (plain HTTP, like the crawl). Mirroring a
  site again overwrites its files. Files over 50 MB are skipped (held in memory before they're written). Links in
  scripts aren't rewritten. A saved page (`monolith`) ignores the delay and `robots.txt`: it's one page the user
  asked for. Tried once on blog.rust-lang.org (3 pages, 40 files, a 2.3 MB saved page).
- Hunt: pages are read as plain HTTP, so images that scripts add (ArtStation, Pinterest, most social sites) aren't
  found; Grab's deep scan is the tool for those. The content finder is a heuristic: it was tried on one real
  hunt ("metin2", 4 pages, 25 content images, no logos or menus); expect misses on unusual layouts. DuckDuckGo
  may refuse or change its page; Hunt then fails with a message and `hunt/search.rs` needs a fix. Searches of a
  pack's sites run one after another with DuckDuckGo's delay, so a pack with many sites is slow.
- Grab's deep scan waits 8 seconds after the page loads and doesn't scroll, so content that loads further down is
  missed. "Grab all" from the extension is tested by its parts (message checks, the socket, the inbox); try it
  once in a browser.

## Rules

The rules agents follow are in `.claude/rules/architecture/`:

- `overview.md`: the modules, where new things go, and how architecture changes are made.
- `code-organization.md`: size limits, section order and dividers.
- `core.md`: scrappy-core's modules and how to add a system, a site, a preset or a source pack.
- `shell.md`: thin commands, capabilities and events.
- `frontend.md`: the display-only window and how to add a view.
- `extension.md`: the popup and the host.
- `security.md`: cookies, secrets, inputs and logs.

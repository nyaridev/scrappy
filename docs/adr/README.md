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
| [0002](0002-external-engines-as-managed-tools.md) | External engines as managed tools | Accepted (lists load as they come: 0030) |
| [0003](0003-editor-safe-encoding-presets.md) | Editor-safe encoding presets | Accepted |
| [0004](0004-library-folder-and-catalog.md) | The library folder and its catalog | Accepted (preferences in the settings file: 0034; changing the folder: 0039) |
| [0005](0005-security-and-secrets.md) | Security and secrets | Accepted |
| [0006](0006-browser-extension-quick-send.md) | Browser extension: a quick-send popup over native messaging | Accepted (host registration: 0014; in-page buttons: 0035) |
| [0007](0007-grab-page-media-discovery.md) | Grab: finding everything downloadable on a page | Accepted (depth, smallest size, duplicates: 0029; saving as found: 0031; browser only: 0032) |
| [0008](0008-crawl-and-offline-mirror.md) | Crawl and offline mirror | Accepted (a site's files moved to Grab: 0031; media picked after the crawl: 0033) |
| [0009](0009-hunt-source-packs-and-dedupe.md) | Hunt: source packs, query detection and duplicate removal | Partly superseded by 0013 |
| [0010](0010-typed-commands-jobs-and-events.md) | Typed commands, jobs and events | Accepted |
| [0011](0011-frameless-window.md) | Frameless window | Accepted |
| [0012](0012-tool-sources-and-pinning.md) | Tool sources and pinning | Accepted (updating yt-dlp at start can be turned off: 0034) |
| [0013](0013-hunt-web-search-and-content-images.md) | Hunt: web search and one content-image finder | Accepted (sources and limits: 0024; image search engines: 0025; sites found through image search: 0026) |
| [0014](0014-builds-installers-and-host-registration.md) | Builds, installers and connecting the browsers | Accepted (installers only, signed updates: 0037; the extension from the release: 0040) |
| [0015](0015-gallery-from-the-library-folder.md) | The Gallery shows the library folder, watched live | Accepted |
| [0016](0016-download-history-and-channel-sections.md) | The Download list is kept; a channel's lists are picked one by one | Accepted (lists load as they come: 0030) |
| [0017](0017-download-sizes-parallel-downloads-and-speed-limit.md) | Sizes before and after, ten downloads at once, a speed limit per download | Accepted |
| [0018](0018-one-downloads-list-in-the-header.md) | One Downloads list from the header, filtered by source; scrapes and hunts get rows | Accepted |
| [0019](0019-grab-scans-as-a-job-and-shows-finds-live.md) | Grab's scan runs as a job and shows its finds as they arrive; finds get thumbnails | Accepted (linked pages: 0029) |
| [0020](0020-editing-hunt-packs-in-the-app.md) | Hunt's packs are edited in the app: name, the whole web, sites added, removed or switched off | Accepted (new, removed and hidden packs: 0028) |
| [0021](0021-gallery-folder-sidebar.md) | The Gallery's folder sidebar replaces the site filter; search runs on each key | Accepted |
| [0022](0022-deleting-from-the-gallery.md) | Right-click Delete on Gallery files and downloaded folders, for good, after a confirmation | Accepted |
| [0023](0023-gallery-viewer-and-player.md) | A viewer for the Gallery with arrow keys, and an in-app video and audio player streamed from Rust | Accepted |
| [0024](0024-hunt-sources-in-parallel.md) | Hunt lists its sources, runs four at once with each one's progress, and limits images per source | Accepted |
| [0025](0025-hunt-image-search-engines.md) | DuckDuckGo, Bing and Yandex image search are Hunt sources, saving full-size images directly | Accepted |
| [0026](0026-hunt-finds-sites-through-image-search.md) | A Hunt site source finds its images through Bing and DuckDuckGo image search, then reads their pages | Accepted |
| [0027](0027-hunt-removes-duplicates.md) | A Hunt setting removes duplicates when it ends, keeping each picture's best copy (strict matching) | Accepted |
| [0028](0028-creating-removing-and-hiding-hunt-packs.md) | Hunt's packs are created, removed (the user's only) and hidden in the app; built-in ones keep their name | Accepted |
| [0029](0029-grab-depth-smallest-size-and-duplicates.md) | Grab follows a page's links to a depth, hides small finds, and removes duplicates after a grab | Accepted (how strictly: 0034) |
| [0030](0030-lists-load-as-they-come.md) | A link's list shows from its first videos, grows as it's listed, and can be stopped | Accepted |
| [0031](0031-grab-takes-scrapes-file-crawl.md) | Grab takes Scrape's file crawl (a fast scan, saving as found); Scrape keeps mirrors and page saves | Accepted (fast scan removed: 0032) |
| [0032](0032-grab-scans-with-the-browser-only.md) | Grab scans with the browser only, to a depth (no Fast, no page limit to set) | Accepted |
| [0033](0033-mirror-pages-first-media-picked-after.md) | A mirror saves its pages first; its images and videos are picked after, with a most video size | Accepted |
| [0034](0034-preferences.md) | Preferences: subtitles and embedding, names, the start preset and quality, a proxy, each view's starting values, duplicate strictness, yt-dlp's updates | Accepted (default name by site: 0042) |
| [0035](0035-in-page-download-buttons.md) | In-page Download buttons on YouTube, SoundCloud, X and Twitch clips, and over any page's video | Accepted (embeds, asking first: 0042) |
| [0036](0036-start-with-the-system.md) | Start with the system, minimized | Accepted (in the tray: 0038; the installer's box: 0041) |
| [0037](0037-updates-from-github-releases.md) | Signed updates from GitHub releases; installers only | Accepted |
| [0038](0038-close-to-the-tray.md) | Close to the tray | Accepted |
| [0039](0039-changing-the-library-folder.md) | Changing the library folder: use another, or move it | Accepted |
| [0040](0040-extension-files-from-the-release.md) | The extension's files from the release for this version | Accepted |
| [0041](0041-start-with-windows-in-the-installer.md) | Start with Windows in the installer, checked | Accepted |
| [0042](0042-extension-reads-the-page-first.md) | The extension reads the page first: the video card, sharing a sign-in, size filters, embeds, videos by site | Accepted |

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
| M14 | Windows, macOS and Linux builds in CI, installers | done |
| M15 | Change the library folder in Settings (another library, or move this one) | done |
| M16 | Extension: in-page Download buttons, the popup's look, its icon | done |
| M17 | Start with the system, updates from GitHub, installers only | done |

### M15: Change the library folder in Settings

- [x] 1. `library`: use another library folder: checked like first-run setup (a new or empty folder gets the layout,
  an existing library keeps its catalog); the settings switch to it, and the Gallery's watcher follows.
- [x] 2. `library`: move this library to a new folder as a job: copy every file and the catalog, check the copy
  (file count and sizes), then switch. The old folder stays where it was until the user removes it.
- [x] 3. Settings: "Use another folder" and "Move library" under Library, with the job's progress, and the old
  folder's path shown after a move. Settings is in tabs now.
- [x] 4. Done checks.

### Later

- The extension is tested by its parts (socket, relay, message checks) and a hello through the real host; load it
  in Chrome and Firefox (`pnpm register-host`, then load `extension/.output/`) to try a real send.
- Decide Scrappy's own licence before the repo goes public (the external tools are GPL; ADR-0002).
- macOS: the overlay title bar (header padding, traffic lights) is untested until there's a Mac build (M14).
- iro-ui 1.0.1: `TextField` lacks `box-sizing: border-box` (its slot-button math assumes it). Fix it there, then
  drop the override in `src/styles.css`.
- Tool pins age: bump them now and then (`core.md` → How to update a tool's pin), then run the ignored install test.
- Linux on ARM and Windows on ARM have no tool downloads yet; they use copies on `PATH`.
- GPU encoders (NVENC, QSV, AMF, VideoToolbox) for the editor-safe re-encode (ADR-0003), after M3's `libx264`.
- Downloads: a Retry button for failed ones; YouTube sometimes answers 403 mid-download, which a retry fixes.
- The Download list is kept in `downloads.db` (ADR-0016); gallery-dl profiles don't offer sections yet.
- The speed limit caps each download, not the total (ADR-0017): a total cap needs a local proxy the tools download
  through. Playlists, channels and profiles show no size before they download.
- Images sent from the extension don't use remembered sign-ins yet (M6).
- Linux builds need the Secret Service (`libdbus`) for the vault's key; the workflows install it.
- Releases are unsigned: Windows SmartScreen and macOS Gatekeeper warn on first start (macOS: right-click → Open).
  Signing and notarizing need certificates (ADR-0014, Later).
- Both workflows passed on their first run (v0.1.0 is a draft release). `pnpm/action-setup@v4` runs on a Node.js
  version GitHub has deprecated; bump it when a newer one is out.
- A real sign-in flow (send → ask → share → download) is tested by its parts; try it once with a signed-in site
  in Chrome and Firefox.
- Collections are tested for real on a YouTube playlist and a Bluesky profile (the ignored test in
  `media/downloads.rs`), and a channel, a playlist and a Mix are checked for real in `media/probes.rs`. X, Instagram and Facebook need a sign-in: try each once through the extension.
- A playlist skips a video that fails and goes on; the finished row doesn't say how many failed yet.
- A send from the extension saves at most the first 50 files of a playlist or profile; the window picks any amount.
- Grab: `vsd` (ADR-0007's stream fallback) isn't added; add it when a stream turns up that yt-dlp can't read.
- The Gallery lists the library folder and reloads when it changes (ADR-0015); a network drive may not report
  changes, so it then updates on the next open.
- Gallery thumbnails are tested with FFmpeg's test pictures; check them once on a real library (HEIC and AVIF depend
  on the FFmpeg build). `.scrappy/thumbs/` keeps a thumbnail after its file is gone.
- Duplicate removal: moving to the system trash is tested by its checks only (a real run would fill the trash of
  the PC running the tests); try it once in the app. It reads JPEG, PNG, WebP, GIF, BMP and TIFF; AVIF and HEIC
  are skipped. A picture edited in place keeps its old hash until it's downloaded again. Every pair is compared,
  which is fine to tens of thousands of pictures.
- Crawl: plain HTTP only; Chrome mode for script-heavy sites waits until a site needs it (ADR-0008). Links are
  sorted by their extension, so a CDN image link without one is missed (an `img` or `video` is listed whatever its
  link). The Scrape view shows the folder; opening the Gallery filtered to it isn't built.
- Mirror: a page's content that scripts build after it loads isn't saved (plain HTTP, like the crawl). Mirroring a
  site again overwrites its pages and stylesheets, keeps its other files, and starts its record over. A page's
  assets over 50 MB are skipped (held in memory before they're written); picked media streams to disk. Media sizes
  show as unknown (no request per file until it's saved), and saving picks ignores the speed limit. Links in
  scripts aren't rewritten. Picking and saving media (ADR-0033) is tested on a local test site only. A saved page
  (`monolith`) ignores the delay, `robots.txt` and the proxy: it's one page the user asked for. Tried once on blog.rust-lang.org (3 pages, 40 files, a 2.3 MB saved page).
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

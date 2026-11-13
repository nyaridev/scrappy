<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="assets/brand/banner-dark.svg" />
    <img src="assets/brand/banner-light.svg" width="100%" alt="Scrappy — keep videos, images and sites from the web" />
  </picture>
</p>

<p align="center">
  <a href="#run"><b>Run</b></a> ·
  <a href="#extension">Extension</a> ·
  <a href="AGENTS.md">Agents</a> ·
  <a href="docs/adr/README.md">Decisions</a>
</p>

A desktop app for keeping things from the web, for Windows, macOS and Linux.

| View | Does |
|---|---|
| **Download** | Saves video and audio from YouTube, X, Instagram, Facebook and any other site: one link, a playlist or a profile. |
| **Grab** | Lists every video, stream, image and file on a page and the pages it links to, to a depth, and saves the ones you pick or all of them as it finds them. |
| **Scrape** | Mirrors a site to read offline, then lets you pick which of its images and videos to save, or saves one page with everything it needs. |
| **Hunt** | Finds images by topic across the web. |
| **Gallery** | Shows everything in the library folder, and removes duplicates. |

No accounts, no telemetry: everything stays in the library folder you pick.

## Run

Needs Node 22.18+, pnpm and Rust.

| Launcher | Does |
|---|---|
| `start-dev.bat` | Opens the app with hot reload. |
| `build.bat` | Builds the Windows app into `release\`. |

Scrappy downloads yt-dlp, Deno, gallery-dl and ffmpeg itself and keeps them up to date.

## Extension

For Chrome, Brave, Edge and Firefox. It adds a Download button beside YouTube's, SoundCloud's, X's and Twitch clips'
own buttons, and over any other page's video, which sends it to the app; Scrappy downloads it with the start preset and
quality from Settings. Its popup lists a page's videos and images to pick from.

```bash
pnpm -C extension build
```

Load `extension/.output/` in the browser as an unpacked extension, then connect it in the app: Settings → Browser
extension.

## Develop

```bash
pnpm install
pnpm check
```

Rust does all the work (`crates/scrappy-core/`); Tauri 2 opens the window, and the React + iro-ui window only shows
what Rust sends. [AGENTS.md](AGENTS.md) has the commands and layout.

## License

MIT. See [LICENSE](LICENSE).

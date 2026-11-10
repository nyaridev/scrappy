# Scrappy

A desktop app for keeping things from the web: download videos from YouTube, X, Instagram, Facebook and any other
site, grab every video, stream and image a page loads, crawl and mirror sites, hunt for images by topic, and browse it
all in one gallery. For Windows, macOS and Linux, with a small Chrome, Brave and Firefox extension that sends a page's
media to the app.

> Work in progress: the scaffold is done, features arrive milestone by milestone (`docs/adr/README.md`).

## Run

Needs Node 22.18+, pnpm and Rust.

- `start-dev.bat`: opens the app with hot reload.
- `build.bat`: builds the portable `release\Scrappy.exe`.

## How it's built

Rust does all the work (`crates/scrappy-core/`); Tauri 2 opens the window, and the React + iro-ui window only shows
what Rust sends. Downloads run on yt-dlp, gallery-dl and ffmpeg, which Scrappy fetches and keeps up to date itself.
The decisions are in [`docs/adr/`](docs/adr/README.md), and [`AGENTS.md`](AGENTS.md) has the commands and layout.

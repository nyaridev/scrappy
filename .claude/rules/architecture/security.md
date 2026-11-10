---
paths:
  - "crates/**"
  - "src-tauri/**"
  - "extension/**"
---

# Security

The reasons are in ADR-0005.

## Cookies and secrets

- Ask for cookies only when a site needs them, and for that one domain. Never read a browser's whole cookie store.
- Grab's deep scan runs the browser with a new, empty profile in a temporary folder, never the user's profile.
- Remembered cookies live only in `vault`, encrypted with the key from the OS keychain. Never in settings, the
  catalog, the library folder or plain files.
- Hand cookies to tools through a temporary file with owner-only permissions, deleted after the run (also on error or
  cancel). Never in arguments, environment variables or `--add-header`. That's `vault::CookieFile`: keep it alive
  until the tool exits, and let it drop.
- Cookies are kept and checked per site (`vault::site_of`); cookies for any other site are refused.
- Never print a cookie's value: `Cookie`'s `Debug` hides it; don't derive or format around that.
- No telemetry, no accounts, no analytics. Requests to third parties carry no user data and use a generic User-Agent.

## Inputs

- Check every argument from the window or the extension in Rust: URLs are `http` or `https` only; paths are resolved
  and must stay inside the library root (or a folder the user picked in a Rust-opened dialog). The library folder
  itself may be typed: `library::check` takes full paths only, and `library::setup` checks again before it creates
  anything.
- Messages from the extension: check size and shape in `bridge` before using anything in them.
- Tools downloaded by `tools` must match their pinned SHA-256 when downloaded; delete a download that doesn't, and
  never unpack or run it. yt-dlp's own updater checks its updates (ADR-0012).

## Logs and errors

- Log through `tracing` only; `logging` redacts cookies, auth headers, secret parameters and email addresses.
- Never log a tool's full command line, request headers or a cookies file's contents. Never run yt-dlp with `-v` in
  normal use.
- Error messages shown to the user say what failed, not the secret values involved.

## Window

- Grant the window no plugin permissions without an ADR-0005 change, and never load a remote URL in it.

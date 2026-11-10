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
- Remembered cookies live only in `vault`, encrypted with the key from the OS keychain. Never in settings, the
  catalog, the library folder or plain files.
- Hand cookies to tools through a temporary file with owner-only permissions, deleted after the run (also on error or
  cancel). Never in arguments, environment variables or `--add-header`.
- No telemetry, no accounts, no analytics. Requests to third parties carry no user data and use a generic User-Agent.

## Inputs

- Check every argument from the window or the extension in Rust: URLs are `http` or `https` only; paths are resolved
  and must stay inside the library root (or a folder the user picked in a Rust-opened dialog).
- Messages from the extension: check size and shape in `bridge` before using anything in them.
- Tools downloaded by `tools` must match their pinned SHA256; refuse to run them otherwise.

## Logs and errors

- Log through `tracing` only; `logging` redacts cookies, auth headers, secret parameters and email addresses.
- Never log a tool's full command line, request headers or a cookies file's contents. Never run yt-dlp with `-v` in
  normal use.
- Error messages shown to the user say what failed, not the secret values involved.

## Window

- Grant the window no plugin permissions without an ADR-0005 change, and never load a remote URL in it.

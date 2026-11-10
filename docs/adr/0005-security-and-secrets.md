# ADR-0005: Security and secrets

**Status:** Accepted
**Date:** 2026-11-10

## Context

Some sites (X, Instagram, Facebook, age-gated YouTube) need the user's cookies. Cookies, tokens and email addresses
must never leak: not into logs, not to other processes, not to other sites. Chrome 127+ encrypts and locks its cookie
database, so reading cookies straight from a browser profile is unreliable on Windows anyway.

## Decision

- **The window has no reach.** Its capability grants Tauri's core defaults only: no fs, shell, http or opener plugin.
  The CSP is strict (`default-src 'self'`, no remote scripts or frames), and the window never loads a remote page.
- **Every input is checked in Rust.** URLs must be http or https. Output paths are resolved and must stay inside the
  library root. Messages from the extension are checked for size and shape (`bridge`).
- **Cookies only when needed, one site at a time.** When a download fails for lack of a session, Scrappy asks. The
  extension sends that one domain's cookies after the user confirms, and the user picks "use once" or "remember".
- **Remembered cookies are encrypted** (`chacha20poly1305`) with a key kept in the OS keychain (`keyring`).
- **Cookies reach the tools through a temporary file** with owner-only permissions, deleted after the run. Never on
  the command line, where other processes can read them.
- **Logs are redacted** (`logging`): cookie and auth headers, secret query parameters and email addresses become
  `[redacted]`. yt-dlp's verbose output is never logged.
- **No telemetry and no accounts.** Requests to search sources use a generic User-Agent.
- **Downloaded tools are checked** against pinned SHA256 sums (ADR-0002).

## Options considered

- **`--cookies-from-browser`:** broken for Chromium on Windows (app-bound encryption, locked database), and it reads
  every site's cookies at once.
- **Cookies in plain files:** simplest, but any program on the PC could read them.

## Consequences

- **Easier:** a leaked log or a copied library folder holds no sessions.
- **Harder:** private content needs the extension (or a cookies.txt import) and a confirmation the first time.
- **Later:** a cookies.txt import for people without the extension goes through the same vault.
- **Rules:** `.claude/rules/architecture/security.md`.

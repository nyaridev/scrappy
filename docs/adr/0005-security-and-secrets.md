# ADR-0005: Security and secrets

**Status:** Accepted; the window's capability is widened by ADR-0011 (window-control permissions), its CSP by ADR-0023 (`media-src` for the app's media protocol); a sign-in can be shared from the popup without a download: ADR-0042
**Date:** 2026-11-10

## Context

Some sites (X, Instagram, Facebook, age-gated YouTube) need the user's cookies. Cookies, tokens and email addresses
must never leak: not into logs, not to other processes, not to other sites. Chrome 127+ encrypts and locks its cookie
database, so reading cookies straight from a browser profile is unreliable on Windows anyway.

## Decision

- **The window has no reach.** Its capability grants Tauri's core defaults, plus moving and closing its own window
  (ADR-0011): no fs, shell, http or opener plugin.
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

## Implementation notes (M6)

- A sign-in belongs to a **site**, the link's registrable domain (`www.youtube.com` → `youtube.com`), with a few
  aliases (`youtu.be` → `youtube.com`, `twitter.com` → `x.com`). The app checks that every cookie it gets belongs
  to the site of the link being sent; the extension can't pick another site.
- The flow: a send fails with a sign-in wall (yt-dlp's error reads like one) → the app replies `needsCookies` →
  the popup asks → the extension reads that site's cookies and sends them with the retry. A failure after
  cookies were given is an error, not another ask. A remembered sign-in is used for that site from then on, by
  the window too; a stale one leads to a new ask, and the new cookies replace it.
- The vault file is `<app data>/vault.bin`; its key is created and stored in the keychain on the first
  "Remember", so nothing touches the keychain before that. If the key is gone from the keychain, the remembered
  sign-ins are lost and the next "Remember" starts a new vault.
- The temporary cookies file is created in the user's temp folder (owner-only on Unix; the folder is private on
  Windows) and deleted when the run ends. Files a crash left behind are removed at the next start.
- `Cookie`'s `Debug` hides the value, and the log redacts cookie-file lines.
- Images the extension sends don't use sign-ins yet.

## Options considered

- **`--cookies-from-browser`:** broken for Chromium on Windows (app-bound encryption, locked database), and it reads
  every site's cookies at once.
- **Cookies in plain files:** simplest, but any program on the PC could read them.

## Consequences

- **Easier:** a leaked log or a copied library folder holds no sessions.
- **Harder:** private content needs the extension (or a cookies.txt import) and a confirmation the first time.
- **Later:** a cookies.txt import for people without the extension goes through the same vault.
- **Rules:** `.claude/rules/architecture/security.md`.

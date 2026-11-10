//! Logging: writes the app log and blanks out secrets on the way, so cookies, tokens, passwords and email addresses
//! never reach a log file (ADR-0005).

// ── Imports ───────────────────────────────
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::Path;
use std::sync::{LazyLock, Mutex};

use regex::Regex;
use tracing_subscriber::fmt::MakeWriter;

// ── Constants ─────────────────────────────
const LOG_FILE: &str = "scrappy.log";
const HIDDEN: &str = "[redacted]";

/// Header values that carry a session: everything after the colon is hidden.
static SECRET_HEADERS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b((?:set-)?cookie|authorization|proxy-authorization|x-csrf-token|x-auth-token)(\s*[:=]\s*)[^\r\n]+")
        .expect("valid regex")
});

/// Query and form values whose names say they're secret: `?token=…`, `&sig=…`, `password=…`.
static SECRET_PARAMS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(token|access_token|refresh_token|sig|signature|key|api_key|apikey|auth|session|sessionid|password|passwd|pwd|secret)=[^&\s]+")
        .expect("valid regex")
});

static EMAILS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}").expect("valid regex"));

// ── Types ─────────────────────────────────
/// Wraps a writer and redacts each chunk before it's written. tracing writes one whole event per call.
pub struct Redacting<W>(W);

/// Hands out `Redacting` writers around the writers of `M`.
pub struct RedactingMakeWriter<M>(M);

impl<W: Write> Write for Redacting<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.write_all(redact(&String::from_utf8_lossy(buf)).as_bytes())?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }
}

impl<'a, M: MakeWriter<'a>> MakeWriter<'a> for RedactingMakeWriter<M> {
    type Writer = Redacting<M::Writer>;

    fn make_writer(&'a self) -> Self::Writer {
        Redacting(self.0.make_writer())
    }
}

// ── Public API ────────────────────────────
/// Returns `text` with cookies, auth headers, secret parameters and email addresses replaced by `[redacted]`.
pub fn redact(text: &str) -> String {
    let text = SECRET_HEADERS.replace_all(text, format!("${{1}}${{2}}{HIDDEN}"));
    let text = SECRET_PARAMS.replace_all(&text, format!("${{1}}={HIDDEN}"));
    EMAILS.replace_all(&text, HIDDEN).into_owned()
}

/// Starts logging to `<log_dir>/scrappy.log` (appending), with every line redacted.
pub fn init(log_dir: &Path) -> Result<(), String> {
    fs::create_dir_all(log_dir).map_err(|e| format!("Can't create the log folder: {e}"))?;
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_dir.join(LOG_FILE))
        .map_err(|e| format!("Can't open the log file: {e}"))?;
    tracing_subscriber::fmt()
        .with_ansi(false)
        .with_writer(RedactingMakeWriter(Mutex::new(file)))
        .try_init()
        .map_err(|e| format!("Logging is already set up: {e}"))?;
    tracing::info!("Scrappy {} started", env!("CARGO_PKG_VERSION"));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::redact;

    #[test]
    fn hides_cookies_and_auth_headers() {
        assert_eq!(redact("Cookie: SID=abc; HSID=def"), "Cookie: [redacted]");
        assert_eq!(redact("set-cookie=a=b"), "set-cookie=[redacted]");
        assert_eq!(redact("Authorization: Bearer xyz\nnext"), "Authorization: [redacted]\nnext");
    }

    #[test]
    fn hides_secret_params_and_keeps_the_rest() {
        assert_eq!(
            redact("https://cdn.example/v.m3u8?token=abc123&quality=hd&sig=ff00"),
            "https://cdn.example/v.m3u8?token=[redacted]&quality=hd&sig=[redacted]"
        );
    }

    #[test]
    fn hides_emails() {
        assert_eq!(redact("signed in as someone@example.com ok"), "signed in as [redacted] ok");
    }

    #[test]
    fn leaves_plain_text_alone() {
        assert_eq!(redact("Downloaded 3 files from youtube.com"), "Downloaded 3 files from youtube.com");
    }
}

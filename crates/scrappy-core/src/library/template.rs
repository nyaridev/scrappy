//! Path templates: turning `Videos/{channel}/{playlist}/{title} [{id}]` and a download's facts into a relative path
//! that's safe on Windows, macOS and Linux (ADR-0004).

// ── Imports ───────────────────────────────
use std::path::PathBuf;
use std::sync::LazyLock;

use regex::{Captures, Regex};

// ── Constants ─────────────────────────────
const MAX_NAME: usize = 120;
const FALLBACK: &str = "untitled";
const FORBIDDEN: &str = r#"<>:"/\|?*"#;
/// Names Windows keeps for devices, with or without an extension.
const RESERVED: [&str; 22] = [
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8", "COM9", "LPT1", "LPT2",
    "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

static PLACEHOLDER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\{(\w+)\}").expect("valid regex"));

// ── Types ─────────────────────────────────
/// The facts a template can use. A missing one renders as nothing, and a folder left empty by it is dropped.
#[derive(Debug, Clone, Default)]
pub struct Fields {
    pub title: Option<String>,
    pub id: Option<String>,
    pub channel: Option<String>,
    pub playlist: Option<String>,
    pub upload_date: Option<String>,
    pub site: Option<String>,
    pub user: Option<String>,
}

// ── Private helpers ───────────────────────
/// `None` for a name templates don't know, so it stays as written.
fn lookup<'a>(fields: &'a Fields, name: &str) -> Option<Option<&'a str>> {
    let value = match name {
        "title" => &fields.title,
        "id" => &fields.id,
        "channel" => &fields.channel,
        "playlist" => &fields.playlist,
        "upload_date" => &fields.upload_date,
        "site" => &fields.site,
        "user" => &fields.user,
        _ => return None,
    };
    Some(value.as_deref())
}

fn fill(part: &str, fields: &Fields) -> String {
    PLACEHOLDER
        .replace_all(part, |caps: &Captures| match lookup(fields, &caps[1]) {
            Some(value) => value.unwrap_or_default().to_string(),
            None => caps[0].to_string(),
        })
        .into_owned()
}

// ── Public API ────────────────────────────
/// Makes one file or folder name safe everywhere: no forbidden or control characters, no trailing dots or spaces,
/// no device names, at most 120 characters, never empty or `..`.
pub fn sanitize(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| match c {
            c if c.is_whitespace() => ' ',
            c if c.is_control() || FORBIDDEN.contains(c) => '_',
            c => c,
        })
        .collect();
    let collapsed = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    let trimmed = collapsed.trim_end_matches(['.', ' ']);
    if trimmed.is_empty() {
        return FALLBACK.into();
    }
    let stem = trimmed.split('.').next().unwrap_or_default();
    let safe = if RESERVED.iter().any(|device| device.eq_ignore_ascii_case(stem.trim_end())) {
        format!("_{trimmed}")
    } else {
        trimmed.to_string()
    };
    let capped: String = safe.chars().take(MAX_NAME).collect();
    capped.trim_end_matches(['.', ' ']).to_string()
}

/// Fills `template` with `fields` and returns a relative path. Each `/`-separated part is filled, then sanitized, so a
/// value can never add folders or climb out of the library.
pub fn render(template: &str, fields: &Fields) -> PathBuf {
    template
        .split(['/', '\\'])
        .map(|part| fill(part, fields))
        .filter(|part| !part.trim().is_empty())
        .map(|part| sanitize(&part))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{Fields, render, sanitize};
    use std::path::PathBuf;

    fn video() -> Fields {
        Fields {
            title: Some("AC/DC: Live?".into()),
            id: Some("abc123".into()),
            channel: Some("Rock  Channel".into()),
            ..Fields::default()
        }
    }

    #[test]
    fn fills_and_drops_empty_folders() {
        let path = render("Videos/{channel}/{playlist}/{title} [{id}]", &video());
        assert_eq!(path, PathBuf::from_iter(["Videos", "Rock Channel", "AC_DC_ Live_ [abc123]"]));
    }

    #[test]
    fn keeps_unknown_placeholders() {
        assert_eq!(render("{nope} {id}", &video()), PathBuf::from("{nope} abc123"));
    }

    #[test]
    fn values_never_escape_the_library() {
        let fields = Fields { title: Some("..".into()), channel: Some("../../etc".into()), ..Fields::default() };
        let path = render("{channel}/{title}", &fields);
        assert_eq!(path, PathBuf::from_iter([".._.._etc", "untitled"]));
    }

    #[test]
    fn sanitizes_names() {
        assert_eq!(sanitize("  a\tb\nc  "), "a b c");
        assert_eq!(sanitize("name. . "), "name");
        assert_eq!(sanitize("con"), "_con");
        assert_eq!(sanitize("LPT1.txt"), "_LPT1.txt");
        assert_eq!(sanitize("console"), "console");
        assert_eq!(sanitize(""), "untitled");
        assert_eq!(sanitize(&"é".repeat(200)).chars().count(), 120);
    }
}

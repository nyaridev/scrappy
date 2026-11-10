//! Route: which tool takes a link. Sites gallery-dl knows best (posts, profiles, image galleries) go to it; every other
//! link goes to yt-dlp, which handles videos, playlists and channels. A new site is one entry in `GALLERY_SITES`.

// ── Imports ───────────────────────────────
use reqwest::Url;

use crate::vault;

// ── Constants ─────────────────────────────
/// Sites whose links go to gallery-dl, with the path part a link must have (`None`: every link on the site).
const GALLERY_SITES: &[(&str, Option<&str>)] = &[
    ("x.com", None),
    ("instagram.com", None),
    ("bsky.app", None),
    ("pixiv.net", None),
    ("deviantart.com", None),
    ("tumblr.com", None),
    ("pinterest.com", None),
    // Facebook's photos only; its videos go to yt-dlp.
    ("facebook.com", Some("/photo")),
];

/// Sites that answer "not found" instead of "sign in" to a visitor who isn't signed in.
const HIDDEN_WITHOUT_SIGN_IN: &[&str] = &["instagram.com", "x.com", "facebook.com"];

// ── Types ─────────────────────────────────
/// The tool a link goes to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Engine {
    YtDlp,
    GalleryDl,
}

// ── Public API ────────────────────────────
/// The tool for `url` (already checked as a web link).
pub(super) fn route(url: &str) -> Engine {
    let (Some(site), Ok(parsed)) = (vault::site_of(url), Url::parse(url)) else { return Engine::YtDlp };
    let gallery = GALLERY_SITES
        .iter()
        .any(|(name, path)| *name == site && path.is_none_or(|part| parsed.path().contains(part)));
    if gallery { Engine::GalleryDl } else { Engine::YtDlp }
}

/// Whether "not found" from `url`'s site, without a sign-in, likely means "sign in".
pub(super) fn hides_without_sign_in(url: &str) -> bool {
    vault::site_of(url).is_some_and(|site| HIDDEN_WITHOUT_SIGN_IN.contains(&site.as_str()))
}

#[cfg(test)]
mod tests {
    use super::{Engine, hides_without_sign_in, route};

    #[test]
    fn sends_social_links_to_gallery_dl() {
        assert_eq!(route("https://x.com/NASA/media"), Engine::GalleryDl);
        assert_eq!(route("https://twitter.com/NASA/status/1"), Engine::GalleryDl);
        assert_eq!(route("https://www.instagram.com/nasa/"), Engine::GalleryDl);
        assert_eq!(route("https://bsky.app/profile/bsky.app"), Engine::GalleryDl);
        assert_eq!(route("https://someone.tumblr.com/post/1"), Engine::GalleryDl);
        assert_eq!(route("https://www.facebook.com/photo/?fbid=1"), Engine::GalleryDl);
    }

    #[test]
    fn sends_the_rest_to_yt_dlp() {
        assert_eq!(route("https://www.youtube.com/@jawed/videos"), Engine::YtDlp);
        assert_eq!(route("https://www.facebook.com/watch/?v=1"), Engine::YtDlp);
        assert_eq!(route("https://vimeo.com/22439234"), Engine::YtDlp);
        assert_eq!(route("https://example.com/video.mp4"), Engine::YtDlp);
    }

    #[test]
    fn knows_sites_that_hide_behind_a_sign_in() {
        assert!(hides_without_sign_in("https://www.instagram.com/nasa/"));
        assert!(!hides_without_sign_in("https://bsky.app/profile/bsky.app"));
    }
}

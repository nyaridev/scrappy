//! Serve: the app's local socket. Each connection is scrappy-host relaying one browser session: a frame in, a reply
//! out, until it closes.

// ── Imports ───────────────────────────────
use std::sync::Arc;

use interprocess::local_socket::tokio::{Stream, prelude::*};
use interprocess::local_socket::{GenericNamespaced, ListenerOptions};

use super::frame;
use super::message::{Checked, MediaKind, Reply, Request, check, check_grab, check_page, check_sign_in};
use crate::grab::Inbox;
use crate::jobs::Jobs;
use crate::media::{Downloads, SendError, Sent};
use crate::settings::Store;
use crate::vault::{self, Vault};

// ── Types ─────────────────────────────────
/// What answering the extension needs.
#[derive(Clone)]
pub struct Bridge {
    pub downloads: Downloads,
    pub jobs: Arc<Jobs>,
    pub settings: Store,
    pub vault: Vault,
    /// Where a page handed to Grab waits for the window.
    pub inbox: Inbox,
}

// ── Private helpers ───────────────────────
async fn answer(bridge: &Bridge, request: Request) -> Reply {
    let (url, kind, page_url, title, headers, cookies, preset) = match request {
        Request::Hello => return Reply::Hello { version: env!("CARGO_PKG_VERSION").into() },
        Request::Grab { page_url, title, finds } => return hand(bridge, &page_url, title.as_deref(), finds),
        Request::Send { url, kind, page_url, title, headers, cookies, preset } => {
            (url, kind, page_url, title, headers, cookies, preset)
        }
        Request::Look { url } => return look(bridge, &url).await,
        Request::Site { url } => return site(bridge, &url),
        Request::SignIn { url, cookies } => return sign_in(bridge, &url, cookies),
    };
    let checked = check(&url, kind, page_url.as_deref(), title.as_deref(), headers, cookies);
    match (checked, bridge.settings.library_root()) {
        (Err(message), _) | (_, Err(message)) => Reply::Error { message },
        (Ok(send), Ok(root)) => queue(bridge, send, root, preset).await,
    }
}

/// The largest thumbnail a look's answer carries, so it stays inside one frame.
const MAX_LOOK_THUMBNAIL: usize = 600 * 1024;

/// What downloading the page would save, as a site reader reads it (ADR-0042).
async fn look(bridge: &Bridge, url: &str) -> Reply {
    let url = match check_page(url) {
        Ok(url) => url,
        Err(message) => return Reply::Error { message },
    };
    match bridge.downloads.probes().look(&url).await {
        Ok(Some(probe)) => Reply::Video {
            title: probe.title,
            channel: probe.channel,
            duration: probe.duration,
            thumbnail: probe.thumbnail.filter(|picture| picture.len() <= MAX_LOOK_THUMBNAIL),
            count: probe.count,
            files: probe.files,
        },
        Ok(None) => Reply::NotVideo,
        Err(SendError::NeedsLogin { site, .. }) => Reply::NeedsCookies { site },
        Err(error) => Reply::Error { message: error.message() },
    }
}

/// The page's site and whether its sign-in is remembered.
fn site(bridge: &Bridge, url: &str) -> Reply {
    match check_page(url).map(|url| vault::site_of(&url)) {
        Ok(Some(site)) => Reply::Site { signed_in: bridge.vault.cookies_for(&site).is_some(), site },
        Ok(None) => Reply::Error { message: "This page has no site to sign in to.".into() },
        Err(message) => Reply::Error { message },
    }
}

/// Remembers a sign-in the user shared from the popup, without a download.
fn sign_in(bridge: &Bridge, url: &str, cookies: Vec<vault::Cookie>) -> Reply {
    let remembered = check_sign_in(url, cookies).and_then(|(site, cookies)| {
        bridge.vault.remember(&site, cookies)?;
        Ok(site)
    });
    match remembered {
        Ok(site) => {
            tracing::info!("The extension shared a sign-in");
            Reply::SignedIn { site }
        }
        Err(message) => Reply::Error { message },
    }
}

/// Hands a checked page to Grab.
fn hand(bridge: &Bridge, page_url: &str, title: Option<&str>, finds: Vec<super::message::Noted>) -> Reply {
    match check_grab(page_url, title, finds) {
        Ok(page) => {
            let count = u32::try_from(page.finds.len()).unwrap_or(u32::MAX);
            bridge.inbox.hand(page);
            tracing::info!("The extension opened a page in Grab");
            Reply::Opened { count }
        }
        Err(message) => Reply::Error { message },
    }
}

/// Queues a checked send, with the preset a button named (checked by `media`). A sign-in wall becomes
/// `NeedsCookies`, unless this send already carried cookies.
async fn queue(bridge: &Bridge, send: Checked, root: std::path::PathBuf, preset: Option<String>) -> Reply {
    let result = match send.kind {
        MediaKind::Image => bridge
            .downloads
            .send_image(&bridge.jobs, root, &send.url, send.page_url, send.title, send.headers)
            .map_err(SendError::Failed),
        MediaKind::Video => {
            let given = send.grant.is_some();
            let cookies = match send.grant {
                Some(grant) if grant.remember => match bridge.vault.remember(&grant.site, grant.cookies.clone()) {
                    Ok(()) => Some(grant.cookies),
                    Err(message) => return Reply::Error { message },
                },
                Some(grant) => Some(grant.cookies),
                None => None,
            };
            let sent = Sent { page_url: send.page_url, title: send.title, preset };
            let result = bridge.downloads.send_video(&bridge.jobs, root, &send.url, sent, send.headers, cookies).await;
            if let Err(SendError::NeedsLogin { site, .. }) = &result
                && !given
            {
                return Reply::NeedsCookies { site: site.clone() };
            }
            result
        }
    };
    match result {
        Ok(entry) => {
            tracing::info!("Queued a download from the extension");
            Reply::Queued { id: entry.id }
        }
        Err(error) => Reply::Error { message: error.message() },
    }
}

async fn handle(connection: Stream, bridge: Bridge) {
    let (mut receive, mut send) = (&connection, &connection);
    loop {
        let body = match frame::read(&mut receive).await {
            Ok(Some(body)) => body,
            Ok(None) => return,
            Err(error) => {
                tracing::warn!("Extension connection dropped: {error}");
                return;
            }
        };
        let reply = match Request::parse(&body) {
            Ok(request) => answer(&bridge, request).await,
            Err(message) => Reply::Error { message },
        };
        if frame::write(&mut send, &reply.to_bytes()).await.is_err() {
            return;
        }
    }
}

// ── Public API ────────────────────────────
/// The socket's name: one per user, so two people on one machine don't share it. scrappy-host builds the same name.
pub fn socket_name() -> String {
    let user = std::env::var("USERNAME").or_else(|_| std::env::var("USER")).unwrap_or_default();
    let user: String = user.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_').collect();
    format!("scrappy-{}.sock", if user.is_empty() { "user" } else { &user })
}

/// Listens for scrappy-host until the app quits. When the socket can't be opened it logs why and returns: the app
/// works on without the extension.
pub async fn serve(bridge: Bridge) {
    if let Err(error) = listen(&socket_name(), bridge).await {
        tracing::warn!("{error}");
    }
}

async fn listen(name: &str, bridge: Bridge) -> Result<(), String> {
    let name = name.to_ns_name::<GenericNamespaced>().map_err(|e| format!("Bad socket name: {e}"))?;
    let listener = ListenerOptions::new()
        .name(name)
        .try_overwrite(true)
        .create_tokio()
        .map_err(|e| format!("Can't listen for the extension: {e}"))?;
    loop {
        match listener.accept().await {
            Ok(connection) => {
                tokio::spawn(handle(connection, bridge.clone()));
            }
            Err(error) => tracing::warn!("Extension connection failed: {error}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Bridge, frame, listen, socket_name};
    use crate::grab::Inbox;
    use crate::jobs::Jobs;
    use crate::media::Downloads;
    use crate::settings::Store;
    use crate::tools::Toolbox;
    use crate::vault::{KeySource, Vault};
    use interprocess::local_socket::tokio::{Stream, prelude::*};
    use interprocess::local_socket::GenericNamespaced;
    use std::sync::Arc;
    use std::time::Duration;

    async fn ask(stream: &Stream, body: &[u8]) -> String {
        let (mut receive, mut send) = (stream, stream);
        frame::write(&mut send, body).await.unwrap();
        String::from_utf8(frame::read(&mut receive).await.unwrap().unwrap()).unwrap()
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn answers_over_the_socket() {
        let dir = tempfile::tempdir().unwrap();
        let vault = Vault::open(&dir.path().join("vault"), KeySource::Fixed([3; 32]));
        let bridge = Bridge {
            downloads: Downloads::new(Toolbox::new(dir.path().join("tools")), vault.clone()),
            jobs: Jobs::new(Arc::new(|_| {})),
            settings: Store::load(dir.path()),
            vault,
            inbox: Inbox::new(Arc::new(|| {})),
        };
        let inbox = bridge.inbox.clone();
        let name = format!("scrappy-test-{}.sock", std::process::id());
        tokio::spawn({
            let name = name.clone();
            async move { listen(&name, bridge).await }
        });
        let mut stream = None;
        for _ in 0..50 {
            if let Ok(connected) = Stream::connect(name.clone().to_ns_name::<GenericNamespaced>().unwrap()).await {
                stream = Some(connected);
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        let stream = stream.expect("the test socket opened");

        assert!(ask(&stream, br#"{"type":"hello"}"#).await.starts_with(r#"{"type":"hello","version":""#));
        assert!(ask(&stream, b"nope").await.contains("couldn't read"));
        let unsafe_link = br#"{"type":"send","url":"file:///C:/x","kind":"image"}"#;
        assert!(ask(&stream, unsafe_link).await.contains("Only http and https"));
        let no_library = br#"{"type":"send","url":"https://a.com/i.png","kind":"image"}"#;
        assert!(ask(&stream, no_library).await.contains("Set up the library first"));
        let grab = br#"{"type":"grab","pageUrl":"https://a.com/","title":"A","finds":[{"url":"https://a.com/v.mp4","mime":null,"size":null}]}"#;
        assert_eq!(ask(&stream, grab).await, r#"{"type":"opened","count":1}"#);
        assert_eq!(inbox.take().unwrap().finds[0].name, "v.mp4");
    }

    #[test]
    fn names_the_socket_per_user() {
        let name = socket_name();
        assert!(name.starts_with("scrappy-") && name.ends_with(".sock"));
        assert!(name.chars().all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c)));
    }
}

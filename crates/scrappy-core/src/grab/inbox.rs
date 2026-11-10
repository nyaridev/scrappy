//! Inbox: the latest page the extension handed to Grab, kept until the window takes it. The window may still be
//! starting when it arrives, so it's kept here rather than only sent as an event.

// ── Imports ───────────────────────────────
use std::sync::{Arc, Mutex, PoisonError};

use super::scan::PageScan;

// ── Types ─────────────────────────────────
/// The handed page and who to tell when one arrives.
#[derive(Clone)]
pub struct Inbox {
    latest: Arc<Mutex<Option<PageScan>>>,
    on_hand: Arc<dyn Fn() + Send + Sync>,
}

// ── Public API ────────────────────────────
impl Inbox {
    /// An empty inbox that calls `on_hand` each time a page arrives.
    pub fn new(on_hand: Arc<dyn Fn() + Send + Sync>) -> Self {
        Self { latest: Arc::default(), on_hand }
    }

    /// Keeps `scan`, replacing an earlier one the window hasn't taken, and tells.
    pub fn hand(&self, scan: PageScan) {
        *self.latest.lock().unwrap_or_else(PoisonError::into_inner) = Some(scan);
        (self.on_hand)();
    }

    /// The handed page, once.
    pub fn take(&self) -> Option<PageScan> {
        self.latest.lock().unwrap_or_else(PoisonError::into_inner).take()
    }
}

#[cfg(test)]
mod tests {
    use super::{Inbox, PageScan};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn hands_a_page_over_once() {
        let told = Arc::new(AtomicUsize::new(0));
        let count = told.clone();
        let inbox = Inbox::new(Arc::new(move || {
            count.fetch_add(1, Ordering::SeqCst);
        }));
        let page = |url: &str| PageScan { url: url.into(), title: None, finds: Vec::new() };
        inbox.hand(page("https://a.com/"));
        inbox.hand(page("https://b.com/"));
        assert_eq!(told.load(Ordering::SeqCst), 2);
        assert_eq!(inbox.take().unwrap().url, "https://b.com/");
        assert!(inbox.take().is_none());
    }
}

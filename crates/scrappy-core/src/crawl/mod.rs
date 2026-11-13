//! Crawl: mirrors a site for offline use with rewritten links, its images and videos picked after the crawl, or saves
//! one page as one file (ADR-0008, ADR-0031, ADR-0033).

mod links;
mod local;
mod mirror;
mod mirrors;
mod page;
mod picks;
mod request;
mod rewrite;
#[cfg(test)]
pub(crate) mod test_site;

pub use mirrors::{MirrorMedia, Mirrors};
pub use picks::{MediaRequest, save_media};
pub use request::{CrawlMode, CrawlRequest, CrawlStarted, crawl};

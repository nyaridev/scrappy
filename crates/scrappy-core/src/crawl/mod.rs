//! Crawl: crawls a site within limits, collecting its files, mirrors it for offline use with rewritten links, or saves
//! one page as one file (ADR-0008).

mod files;
mod links;
mod local;
mod mirror;
mod page;
mod request;
mod rewrite;
#[cfg(test)]
pub(crate) mod test_site;

pub use links::file_links;
pub use request::{CrawlMode, CrawlRequest, CrawlStarted, crawl};

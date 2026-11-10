//! Vault: browser sign-ins (cookies), asked for only when a site needs them, used once or remembered encrypted with a
//! key from the OS keychain, and handed to tools through a temporary owner-only file (ADR-0005).

mod cookie;
mod site;
mod store;

pub use cookie::{Cookie, CookieFile, check_cookies, netscape};
pub use site::{belongs_to, site_of};
pub use store::{KeySource, RememberedSite, Vault};

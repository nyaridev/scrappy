//! Bridge: the extension's way in (ADR-0006). The browser starts scrappy-host, which relays frames to the app's local
//! socket; here they're read, checked and turned into downloads, and the browsers are connected to the host
//! (ADR-0014).

mod frame;
mod message;
mod register;
mod serve;

pub use message::{Checked, CookieGrant, Grant, Header, MediaKind, Reply, Request, check};
pub use register::{HostStatus, connect, disconnect, host_status};
pub use serve::{Bridge, serve, socket_name};

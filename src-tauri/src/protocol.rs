//! Protocol: the `scrappy:` app protocol the Gallery's player streams videos and audio from (ADR-0023). Core checks the
//! path and reads the range; this turns its answer into an HTTP response.

use scrappy_core::library;
use scrappy_core::settings::Store;
use tauri::http::{Request, Response, header};
use tauri::{AppHandle, Manager};

/// The answer to one request from the player.
pub fn media(app: &AppHandle, request: &Request<Vec<u8>>) -> Response<Vec<u8>> {
    let range = request.headers().get(header::RANGE).and_then(|value| value.to_str().ok());
    let served = app.state::<Store>().library_root().and_then(|root| library::serve(&root, request.uri().path(), range));
    let response = match served {
        Ok(served) => Response::builder()
            .status(served.status)
            .header(header::CONTENT_TYPE, served.mime)
            .header(header::CONTENT_RANGE, served.range)
            .header(header::ACCEPT_RANGES, "bytes")
            .body(served.body),
        // Outside the library, not video or audio, or gone: the player shows it can't play.
        Err(_) => Response::builder().status(404).body(Vec::new()),
    };
    response.unwrap_or_else(|_| Response::new(Vec::new()))
}

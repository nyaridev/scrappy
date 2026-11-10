//! Relay: frames from the browser go to the app, each reply comes back. Native messaging's framing on both sides: a
//! 4-byte length in native byte order, then the JSON.

// ── Imports ───────────────────────────────
use std::io::{self, Read, Write};

// ── Constants ─────────────────────────────
/// The same limit as the app's (`scrappy-core::bridge`); Chrome's own limit for host replies is 1 MB.
const MAX_FRAME: usize = 1024 * 1024;
/// What the popup gets when the app can't be reached; it reads it like any reply.
const NO_APP: &[u8] = br#"{"type":"error","message":"Scrappy didn't start."}"#;

// ── Private helpers ───────────────────────
fn read_frame(reader: &mut impl Read) -> io::Result<Option<Vec<u8>>> {
    let mut length = [0u8; 4];
    match reader.read_exact(&mut length) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e),
    }
    let length = u32::from_ne_bytes(length) as usize;
    if length > MAX_FRAME {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "frame too large"));
    }
    let mut body = vec![0u8; length];
    reader.read_exact(&mut body)?;
    Ok(Some(body))
}

fn write_frame(writer: &mut impl Write, body: &[u8]) -> io::Result<()> {
    let length = u32::try_from(body.len())
        .ok()
        .filter(|_| body.len() <= MAX_FRAME)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "frame too large"))?;
    writer.write_all(&length.to_ne_bytes())?;
    writer.write_all(body)?;
    writer.flush()
}

// ── Public API ────────────────────────────
/// Relays until the browser closes stdin. Connects to the app with `connect` on the first frame, and again after a
/// failure, so a later frame can still get through.
pub fn relay<A: Read + Write>(
    mut browser_in: impl Read,
    mut browser_out: impl Write,
    mut connect: impl FnMut() -> io::Result<A>,
) -> io::Result<()> {
    let mut app: Option<A> = None;
    while let Some(request) = read_frame(&mut browser_in)? {
        if app.is_none() {
            app = connect().ok();
        }
        let reply = match app.as_mut() {
            Some(stream) => write_frame(stream, &request).and_then(|()| read_frame(stream)),
            None => Ok(None),
        };
        match reply {
            Ok(Some(reply)) => write_frame(&mut browser_out, &reply)?,
            _ => {
                app = None;
                write_frame(&mut browser_out, NO_APP)?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{NO_APP, read_frame, relay, write_frame};
    use interprocess::local_socket::{GenericNamespaced, ListenerOptions, Stream, prelude::*};
    use std::io::{self, Read, Write};
    use std::thread;

    fn frames(bodies: &[&[u8]]) -> Vec<u8> {
        let mut bytes = Vec::new();
        for body in bodies {
            write_frame(&mut bytes, body).unwrap();
        }
        bytes
    }

    fn replies(mut bytes: &[u8]) -> Vec<Vec<u8>> {
        std::iter::from_fn(|| read_frame(&mut bytes).unwrap()).collect()
    }

    #[test]
    fn relays_to_a_fake_app() {
        let name = format!("scrappy-host-test-{}.sock", std::process::id());
        let socket = || name.clone().to_ns_name::<GenericNamespaced>();
        let listener = ListenerOptions::new().name(socket().unwrap()).create_sync().unwrap();
        // The fake app answers each frame with the frame's length.
        let app = thread::spawn(move || {
            let mut stream = listener.accept().unwrap();
            while let Some(body) = read_frame(&mut stream).unwrap() {
                write_frame(&mut stream, body.len().to_string().as_bytes()).unwrap();
            }
        });
        let input = frames(&[br#"{"type":"hello"}"#, b"{}"]);
        let mut output = Vec::new();
        relay(input.as_slice(), &mut output, || Stream::connect(socket()?)).unwrap();
        assert_eq!(replies(&output), [b"16".to_vec(), b"2".to_vec()]);
        app.join().unwrap();
    }

    /// An app connection that closes at once.
    struct Closed;

    impl Read for Closed {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
            Ok(0)
        }
    }

    impl Write for Closed {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::ErrorKind::BrokenPipe.into())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn answers_for_an_app_that_isnt_there() {
        let input = frames(&[b"{}", b"{}"]);
        let mut output = Vec::new();
        let mut tries = 0;
        relay(input.as_slice(), &mut output, || {
            tries += 1;
            if tries == 1 { Err(io::ErrorKind::NotFound.into()) } else { Ok(Closed) }
        })
        .unwrap();
        assert_eq!(replies(&output), [NO_APP.to_vec(), NO_APP.to_vec()]);
        assert_eq!(tries, 2);
    }

    #[test]
    fn refuses_a_huge_frame() {
        let bytes = (2u32 * 1024 * 1024).to_ne_bytes();
        assert!(read_frame(&mut bytes.as_slice()).is_err());
    }
}

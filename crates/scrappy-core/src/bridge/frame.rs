//! Frame: native messaging's framing, used end to end: a 4-byte length in native byte order, then that much JSON.

// ── Imports ───────────────────────────────
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

// ── Constants ─────────────────────────────
/// The largest frame either side takes; Chrome's own limit for host replies is 1 MB.
pub const MAX_FRAME: usize = 1024 * 1024;

// ── Public API ────────────────────────────
/// Reads one frame; `None` when the other side closed cleanly between frames.
pub async fn read<R: AsyncRead + Unpin>(reader: &mut R) -> Result<Option<Vec<u8>>, String> {
    let mut length = [0u8; 4];
    match reader.read_exact(&mut length).await {
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(format!("Can't read from the extension: {e}")),
    }
    let length = u32::from_ne_bytes(length) as usize;
    if length > MAX_FRAME {
        return Err("The message is too large.".into());
    }
    let mut body = vec![0u8; length];
    reader.read_exact(&mut body).await.map_err(|e| format!("Can't read from the extension: {e}"))?;
    Ok(Some(body))
}

/// Writes one frame.
pub async fn write<W: AsyncWrite + Unpin>(writer: &mut W, body: &[u8]) -> Result<(), String> {
    let length = u32::try_from(body.len()).ok().filter(|_| body.len() <= MAX_FRAME).ok_or("The reply is too large.")?;
    writer.write_all(&length.to_ne_bytes()).await.map_err(|e| format!("Can't answer the extension: {e}"))?;
    writer.write_all(body).await.map_err(|e| format!("Can't answer the extension: {e}"))?;
    writer.flush().await.map_err(|e| format!("Can't answer the extension: {e}"))
}

#[cfg(test)]
mod tests {
    use super::{MAX_FRAME, read, write};

    #[tokio::test]
    async fn round_trips_and_stops_at_the_end() {
        let mut buffer = Vec::new();
        write(&mut buffer, br#"{"type":"hello"}"#).await.unwrap();
        write(&mut buffer, b"{}").await.unwrap();
        let mut reader = buffer.as_slice();
        assert_eq!(read(&mut reader).await.unwrap().unwrap(), br#"{"type":"hello"}"#);
        assert_eq!(read(&mut reader).await.unwrap().unwrap(), b"{}");
        assert_eq!(read(&mut reader).await.unwrap(), None);
    }

    #[tokio::test]
    async fn refuses_a_huge_frame() {
        let mut bytes = ((MAX_FRAME + 1) as u32).to_ne_bytes().to_vec();
        bytes.extend_from_slice(b"x");
        assert!(read(&mut bytes.as_slice()).await.unwrap_err().contains("too large"));
    }
}

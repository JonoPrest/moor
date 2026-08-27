//! Length-prefixed JSON frames.
//!
//! Wire layout: `u32` big-endian payload length, then that many bytes of JSON
//! encoding one [`Envelope`]. Frames above [`MAX_FRAME`] are refused before
//! any allocation so a hostile peer cannot ask for gigabytes.

use moor_protocol::Envelope;
use serde::Serialize;
use serde::de::DeserializeOwned;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

/// Largest payload accepted or produced, in bytes. A 20k-line render chunk
/// stream never approaches this; it exists to bound memory per connection.
pub const MAX_FRAME: u32 = 64 * 1024 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum CodecError {
    #[error("frame of {len} bytes exceeds the {MAX_FRAME} byte limit")]
    Oversized { len: u32 },
    #[error("malformed frame: {0}")]
    Json(#[from] serde_json::Error),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

/// Serialise an envelope to a frame payload.
pub fn encode<T: Serialize>(env: &Envelope<T>) -> Result<Vec<u8>, CodecError> {
    let bytes = serde_json::to_vec(env)?;
    if bytes.len() > MAX_FRAME as usize {
        return Err(CodecError::Oversized {
            len: u32::try_from(bytes.len()).unwrap_or(u32::MAX),
        });
    }
    Ok(bytes)
}

/// Parse a frame payload into an envelope.
pub fn decode<T: DeserializeOwned>(bytes: &[u8]) -> Result<Envelope<T>, CodecError> {
    Ok(serde_json::from_slice(bytes)?)
}

/// Read one frame. `Ok(None)` on a clean EOF at a frame boundary; an EOF in
/// the middle of a frame is an error.
pub async fn read_frame<R: AsyncRead + Unpin>(r: &mut R) -> Result<Option<Vec<u8>>, CodecError> {
    let mut len = [0u8; 4];
    match r.read_exact(&mut len).await {
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e.into()),
    }
    let len = u32::from_be_bytes(len);
    if len > MAX_FRAME {
        return Err(CodecError::Oversized { len });
    }
    let mut buf = vec![0u8; len as usize];
    r.read_exact(&mut buf).await?;
    Ok(Some(buf))
}

/// Write one frame and flush.
pub async fn write_frame<W: AsyncWrite + Unpin>(
    w: &mut W,
    payload: &[u8],
) -> Result<(), CodecError> {
    let len = u32::try_from(payload.len())
        .ok()
        .filter(|l| *l <= MAX_FRAME)
        .ok_or(CodecError::Oversized {
            len: u32::try_from(payload.len()).unwrap_or(u32::MAX),
        })?;
    w.write_all(&len.to_be_bytes()).await?;
    w.write_all(payload).await?;
    w.flush().await?;
    Ok(())
}

/// Read a frame and decode it. `Ok(None)` on clean EOF.
pub async fn read_msg<R: AsyncRead + Unpin, T: DeserializeOwned>(
    r: &mut R,
) -> Result<Option<Envelope<T>>, CodecError> {
    match read_frame(r).await? {
        Some(bytes) => Ok(Some(decode(&bytes)?)),
        None => Ok(None),
    }
}

/// Encode an envelope and write it as one frame.
pub async fn write_msg<W: AsyncWrite + Unpin, T: Serialize>(
    w: &mut W,
    env: &Envelope<T>,
) -> Result<(), CodecError> {
    write_frame(w, &encode(env)?).await
}

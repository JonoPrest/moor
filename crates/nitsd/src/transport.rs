//! Framed transports. The wire format is one JSON [`Envelope`] per frame;
//! how frames are delimited is the transport's business:
//!
//! * byte streams (unix socket, stdio) use a u32 big-endian length prefix
//!   ([`codec::read_frame`] / [`codec::write_frame`]);
//! * WebSocket carries one envelope per binary (or text) message, which is
//!   what a browser client can speak directly.
//!
//! [`Envelope`]: nits_protocol::Envelope

use std::future::Future;

use futures_util::stream::{SplitSink, SplitStream};
use futures_util::{SinkExt, StreamExt};
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt, BufReader, BufWriter, ReadHalf, WriteHalf};
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::tungstenite::{self, Message};

use crate::codec::{self, CodecError};

/// Receiving half of a framed transport.
pub trait FrameRead: Send + Unpin {
    /// Next frame, or `None` when the peer closed cleanly.
    fn recv(&mut self) -> impl Future<Output = Result<Option<Vec<u8>>, CodecError>> + Send;
}

/// Sending half of a framed transport.
pub trait FrameWrite: Send + Unpin {
    fn send(&mut self, frame: &[u8]) -> impl Future<Output = Result<(), CodecError>> + Send;
    /// Flush and signal end-of-stream to the peer.
    fn close(&mut self) -> impl Future<Output = Result<(), CodecError>> + Send;
}

/// Length-prefixed frames over a byte stream.
pub type ByteRead<S> = BufReader<ReadHalf<S>>;
/// Length-prefixed frames over a byte stream.
pub type ByteWrite<S> = BufWriter<WriteHalf<S>>;

/// Split a byte stream into length-prefixed framed halves.
pub fn byte_stream<S>(stream: S) -> (ByteRead<S>, ByteWrite<S>)
where
    S: AsyncRead + AsyncWrite + Send + 'static,
{
    let (rd, wr) = tokio::io::split(stream);
    (BufReader::new(rd), BufWriter::new(wr))
}

impl<S: AsyncRead + Send + 'static> FrameRead for ByteRead<S> {
    async fn recv(&mut self) -> Result<Option<Vec<u8>>, CodecError> {
        codec::read_frame(self).await
    }
}

impl<S: AsyncWrite + Send + 'static> FrameWrite for ByteWrite<S> {
    async fn send(&mut self, frame: &[u8]) -> Result<(), CodecError> {
        codec::write_frame(self, frame).await
    }
    async fn close(&mut self) -> Result<(), CodecError> {
        self.shutdown().await?;
        Ok(())
    }
}

/// Receiving half of a WebSocket.
pub type WsRead<S> = SplitStream<WebSocketStream<S>>;
/// Sending half of a WebSocket.
pub type WsWrite<S> = SplitSink<WebSocketStream<S>, Message>;

/// Split an accepted/connected WebSocket into framed halves.
pub fn web_socket<S>(ws: WebSocketStream<S>) -> (WsRead<S>, WsWrite<S>)
where
    S: AsyncRead + AsyncWrite + Send + Unpin + 'static,
{
    let (tx, rx) = ws.split();
    (rx, tx)
}

fn ws_error(e: tungstenite::Error) -> CodecError {
    match e {
        tungstenite::Error::Io(io) => CodecError::Io(io),
        tungstenite::Error::ConnectionClosed | tungstenite::Error::AlreadyClosed => {
            CodecError::Io(std::io::Error::from(std::io::ErrorKind::UnexpectedEof))
        }
        other => CodecError::Io(std::io::Error::other(other)),
    }
}

impl<S: AsyncRead + AsyncWrite + Send + Unpin + 'static> FrameRead for WsRead<S> {
    async fn recv(&mut self) -> Result<Option<Vec<u8>>, CodecError> {
        loop {
            match self.next().await {
                None
                | Some(
                    Ok(Message::Close(_))
                    | Err(
                        tungstenite::Error::ConnectionClosed
                        | tungstenite::Error::AlreadyClosed
                        | tungstenite::Error::Protocol(
                            tungstenite::error::ProtocolError::ResetWithoutClosingHandshake,
                        ),
                    ),
                ) => return Ok(None),
                Some(Ok(Message::Binary(b))) => {
                    if b.len() > codec::MAX_FRAME as usize {
                        return Err(CodecError::Oversized {
                            len: u32::try_from(b.len()).unwrap_or(u32::MAX),
                        });
                    }
                    return Ok(Some(b.to_vec()));
                }
                Some(Ok(Message::Text(t))) => return Ok(Some(t.as_bytes().to_vec())),
                // Pings are answered by tungstenite on the next write.
                Some(Ok(Message::Ping(_) | Message::Pong(_) | Message::Frame(_))) => {}
                Some(Err(e)) => return Err(ws_error(e)),
            }
        }
    }
}

impl<S: AsyncRead + AsyncWrite + Send + Unpin + 'static> FrameWrite for WsWrite<S> {
    async fn send(&mut self, frame: &[u8]) -> Result<(), CodecError> {
        if frame.len() > codec::MAX_FRAME as usize {
            return Err(CodecError::Oversized {
                len: u32::try_from(frame.len()).unwrap_or(u32::MAX),
            });
        }
        SinkExt::send(self, Message::Binary(frame.to_vec().into()))
            .await
            .map_err(ws_error)
    }
    async fn close(&mut self) -> Result<(), CodecError> {
        match SinkExt::close(self).await {
            Ok(())
            | Err(tungstenite::Error::ConnectionClosed | tungstenite::Error::AlreadyClosed) => {
                Ok(())
            }
            Err(e) => Err(ws_error(e)),
        }
    }
}

/// Read and decode one envelope.
pub async fn recv_msg<R: FrameRead, T: serde::de::DeserializeOwned>(
    rd: &mut R,
) -> Result<Option<nits_protocol::Envelope<T>>, CodecError> {
    match rd.recv().await? {
        Some(bytes) => Ok(Some(codec::decode(&bytes)?)),
        None => Ok(None),
    }
}

/// Encode and send one envelope.
pub async fn send_msg<W: FrameWrite, T: serde::Serialize>(
    wr: &mut W,
    env: &nits_protocol::Envelope<T>,
) -> Result<(), CodecError> {
    wr.send(&codec::encode(env)?).await
}

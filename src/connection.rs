// Copyright (c) 2019 Parity Technologies (UK) Ltd.
//
// Licensed under the Apache License, Version 2.0
// <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0> or the MIT
// license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. All files in the project carrying such notice may not be copied,
// modified, or distributed except according to those terms.

//! A persistent websocket connection after the handshake phase.

mod compat;
mod data;
mod frame;

use bytes::BytesMut;
use compat::AioCompat;
use crate::{base::{self, Header, OpCode}, extension::Extension};
use frame::{Frame, FrameCodec};
use log::{debug, trace, warn};
use futures::{prelude::*, ready};
use smallvec::SmallVec;
use std::{fmt, io, pin::Pin, task::{Context, Poll}};

pub use data::Data;

/// Accumulated max. size of a complete message.
const MAX_MESSAGE_SIZE: usize = 256 * 1024 * 1024;
/// Max. size of a single message frame.
const MAX_FRAME_SIZE: usize = MAX_MESSAGE_SIZE;

type Result<T> = std::result::Result<T, Error>;

/// Is the [`Connection`] used by a client or server?
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Client-side of a connection (implies masking of payload data).
    Client,
    /// Server-side of a connection.
    Server
}

impl Mode {
    pub fn is_client(self) -> bool {
        if let Mode::Client = self {
            true
        } else {
            false
        }
    }

    pub fn is_server(self) -> bool {
        !self.is_client()
    }
}

/// A persistent, bidirectional websocket connection.
///
/// After the successful websocket handshake one can create a
/// `Connection` from [`crate::handshake::Client::into_connection`] or
/// [`crate::handshake::Server::into_connection`].
///
/// # Control messages
///
/// The remote may test the connection's health by sending ping messages
/// which must be answered, or else the connection would probably be closed.
/// In order to do so it is important to not only use this connection as a
/// [`Sink`] but also read incoming data using the [`Stream`] interface.
/// Upon reading, incoming pings will automatically be answered, but without
/// any reading, no control message will be processed.
#[derive(Debug)]
pub struct Connection<T> {
    mode: Mode,
    framed: tokio_codec::Framed<AioCompat<T>, FrameCodec>,
    state: Option<State>,
    extensions: SmallVec<[Box<dyn Extension + Send>; 4]>,
    max_message_size: usize,
    validate_utf8: bool
}

impl<T: AsyncRead + AsyncWrite + Unpin> Connection<T> {
    /// Create a new `Connection` from the given socket.
    ///
    /// The `mode` sets this connection into client or server mode which
    /// has implications w.r.t. the masking of payload data.
    ///
    /// The `buf` parameter denotes the read buffer from the handshake phase
    /// which may contain further unprocessed data but may also be empty.
    pub(crate) fn new(socket: T, mode: Mode, buf: BytesMut) -> Self {
        let codec = {
            let mut codec = base::Codec::default();
            codec.set_max_data_size(MAX_FRAME_SIZE);
            FrameCodec { codec, header: None }
        };

        let framed = {
            let mut parts = tokio_codec::Framed::new(AioCompat(socket), codec).into_parts();
            parts.read_buf = buf;
            tokio_codec::Framed::from_parts(parts)
        };

        Connection {
            mode,
            framed,
            state: Some(State::Open(None)),
            extensions: SmallVec::new(),
            max_message_size: MAX_MESSAGE_SIZE,
            validate_utf8: true
        }
    }

    /// Add extensions to this connection.
    ///
    /// Only enabled extensions will be considered.
    pub fn add_extensions<I>(&mut self, extensions: I) -> &mut Self
    where
        I: IntoIterator<Item = Box<dyn Extension + Send>>
    {
        for e in extensions.into_iter().filter(|e| e.is_enabled()) {
            debug!("using extension: {}", e.name());
            self.framed.codec_mut().codec.add_reserved_bits(e.reserved_bits());
            self.extensions.push(e)
        }
        self
    }

    /// Set the maximum size of a complete message.
    ///
    /// Message fragments will be buffered and concatenated up to this value,
    /// i.e. the sum of all message frames payload lengths will not be greater
    /// than this maximum. However, extensions may increase the total message
    /// size further, e.g. by decompressing the payload data.
    pub fn set_max_message_size(&mut self, max: usize) -> &mut Self {
        self.max_message_size = max;
        self
    }

    /// Set the maximum size of a single websocket frame payload.
    pub fn set_max_frame_size(&mut self, max: usize) -> &mut Self {
        self.framed.codec_mut().codec.set_max_data_size(max);
        self
    }

    /// Toggle UTF-8 check for incoming text messages.
    ///
    /// By default all text frames are validated.
    pub fn validate_utf8(&mut self, value: bool) -> &mut Self {
        self.validate_utf8 = value;
        self
    }

    /// Pin projection for `self.framed`.
    fn framed(self: Pin<&mut Self>) -> Pin<&mut tokio_codec::Framed<AioCompat<T>, FrameCodec>> {
        Pin::new(&mut self.get_mut().framed)
    }

    /// If we are a websocket client, create and apply a mask.
    fn apply_mask(&self, frame: &mut Frame) {
        if self.mode.is_client() {
            frame.header_mut().set_masked(true);
            frame.header_mut().set_mask(rand::random());
            let (header, payload) = frame.as_mut();
            base::Codec::apply_mask(header, payload)
        }
    }

    /// Apply all extensions to the given header and payload data.
    fn decode_with_extensions(&mut self, h: &mut Header, d: &mut BytesMut) -> Result<()> {
        for e in &mut self.extensions {
            trace!("decoding with extension: {}", e.name());
            e.decode(h, d).map_err(Error::Extension)?
        }
        Ok(())
    }

    /// Apply all extensions to the given header and payload data.
    fn encode_with_extensions(&mut self, h: &mut Header, d: &mut BytesMut) -> Result<()> {
        for e in &mut self.extensions {
            trace!("encoding with extension: {}", e.name());
            e.encode(h, d).map_err(Error::Extension)?
        }
        Ok(())
    }

    /// Check that we stay below the configured max. message size.
    fn ensure_max_message_size(&self, current: &[u8], new: &[u8]) -> Result<()> {
        let size = current.len() + new.len();
        if size > self.max_message_size {
            let e = Error::MessageTooLarge { current: size, maximum: self.max_message_size };
            warn!("{}", e);
            return Err(e)
        }
        Ok(())
    }

    fn answer_ping(mut self: Pin<&mut Self>, c: &mut Context, f: Frame, d: Option<Data>) -> Poll<Result<()>> {
        trace!("answering ping: {}", f.header());
        match self.as_mut().framed().poll_ready(c) {
            Poll::Pending => {
                self.state = Some(State::AnswerPing(f, d));
                Poll::Pending
            }
            Poll::Ready(Ok(())) => {
                self.as_mut().framed().start_send(f)?;
                self.flush(c, d)
            }
            Poll::Ready(Err(e)) => Poll::Ready(Err(e))
        }
    }

    fn answer_close(mut self: Pin<&mut Self>, c: &mut Context, f: Frame) -> Poll<Result<()>> {
        trace!("answering close: {}", f.header());
        match self.as_mut().framed().poll_ready(c) {
            Poll::Pending => {
                self.state = Some(State::AnswerClose(f));
                Poll::Pending
            }
            Poll::Ready(Ok(())) => {
                self.as_mut().framed().start_send(f)?;
                self.closing(c)
            }
            Poll::Ready(Err(e)) => Poll::Ready(Err(e))
        }
    }

    fn closing(mut self: Pin<&mut Self>, c: &mut Context) -> Poll<Result<()>> {
        trace!("closing");
        match self.as_mut().framed().poll_close(c) {
            Poll::Pending => {
                self.state = Some(State::Closing);
                Poll::Pending
            }
            Poll::Ready(Ok(())) => {
                self.state = Some(State::Closed);
                Poll::Ready(Ok(()))
            }
            Poll::Ready(Err(e)) => Poll::Ready(Err(e))
        }
    }

    fn flush(mut self: Pin<&mut Self>, c: &mut Context, d: Option<Data>) -> Poll<Result<()>> {
        trace!("flushing");
        match self.as_mut().framed().poll_flush(c) {
            Poll::Pending => {
                self.state = Some(State::Flush(d));
                Poll::Pending
            }
            Poll::Ready(Ok(())) => {
                self.state = Some(State::Open(d));
                Poll::Ready(Ok(()))
            }
            Poll::Ready(Err(e)) => Poll::Ready(Err(e))
        }
    }

    fn send_close(mut self: Pin<&mut Self>, c: &mut Context, f: Frame) -> Poll<Result<()>> {
        trace!("sending close: {}", f.header());
        match self.as_mut().framed().poll_ready(c) {
            Poll::Pending => {
                self.state = Some(State::SendClose(f));
                Poll::Pending
            }
            Poll::Ready(Ok(())) => {
                self.as_mut().framed().start_send(f)?;
                self.flush_close(c)
            }
            Poll::Ready(Err(e)) => Poll::Ready(Err(e))
        }
    }

    fn flush_close(mut self: Pin<&mut Self>, c: &mut Context) -> Poll<Result<()>> {
        trace!("flushing close");
        match self.as_mut().framed().poll_close(c) {
            Poll::Pending => {
                self.state = Some(State::FlushClose);
                Poll::Pending
            }
            Poll::Ready(Ok(())) => {
                self.state = Some(State::AwaitClose);
                Poll::Ready(Ok(()))
            }
            Poll::Ready(Err(e)) => Poll::Ready(Err(e))
        }
    }

    fn await_close(mut self: Pin<&mut Self>, c: &mut Context) -> Poll<Result<()>> {
        trace!("awaiting close");
        loop {
            match self.as_mut().framed().poll_next(c) {
                Poll::Pending => {
                    self.state = Some(State::AwaitClose);
                    return Poll::Pending
                }
                Poll::Ready(None) => {
                    self.state = Some(State::Closed);
                    return Poll::Ready(Ok(()))
                }
                Poll::Ready(Some(Ok(frame))) =>
                    if OpCode::Close == frame.header().opcode() {
                        self.state = Some(State::Closed);
                        return Poll::Ready(Ok(()))
                    }
                Poll::Ready(Some(Err(e))) => return Poll::Ready(Err(e))
            }
        }
    }
}

/// Connection state which reflects the current processing state.
#[derive(Debug)]
enum State {
    /// Default state.
    /// Possible transitions: `Open`, `AnswerPing`, `AnswerClose`, `Closed`.
    Open(Option<Data>),

    /// Send a PONG frame as answer to a PING we have received.
    /// Possible transitions: `AnswerPing`, `Open`.
    AnswerPing(Frame, Option<Data>),

    /// Flush some frame we started sending.
    /// Possible transitions: `Flush`, `Open`.
    Flush(Option<Data>),

    /// We want to send a close frame.
    /// Possible transitions: `SendClose`, `FlushClose`.
    SendClose(Frame),

    /// We have sent a close frame and need to flush it.
    /// Possible transitions: `FlushClose`, `AwaitClose`.
    FlushClose,

    /// We have sent a close frame and awaiting a close response.
    /// Possible transitions: `AwaitClose`, `Closed`.
    AwaitClose,

    /// We have received a close frame and want to send a close response.
    /// Possible transitions: `AnswerClose`, `Closing`.
    AnswerClose(Frame),

    /// We have begun sending a close answer frame and need to flush it.
    /// Possible transitions: `Closing`, `Closed`.
    Closing,

    /// We are closed (terminal state).
    /// Possible transitions: none.
    Closed
}

// Small helper to lift an error into a `Poll` value.
macro_rules! lift {
    ($e: expr) => {
        if let Err(e) = $e {
            return Poll::Ready(Some(Err(e.into())))
        }
    }
}

impl<T: AsyncRead + AsyncWrite + Unpin> Stream for Connection<T> {
    type Item = Result<Data>;

    fn poll_next(mut self: Pin<&mut Self>, ctx: &mut Context) -> Poll<Option<Self::Item>> {
        loop {
            match self.as_mut().state.take() {
                Some(State::Open(None)) => match self.as_mut().framed().poll_next(ctx) {
                    Poll::Ready(Some(Ok(mut frame))) => {
                        trace!("received: {}", frame.header());
                        match frame.header().opcode() {
                            OpCode::Text | OpCode::Binary if frame.header().is_fin() => {
                                self.state = Some(State::Open(None));
                                let (header, payload) = frame.as_mut();
                                lift!(self.decode_with_extensions(header, payload));
                                let cons =
                                    if frame.is_text() {
                                        if self.validate_utf8 {
                                            lift!(std::str::from_utf8(frame.payload()))
                                        }
                                        Data::Text
                                    } else {
                                        Data::Binary
                                    };
                                return Poll::Ready(Some(Ok(cons(frame.into_payload()))))
                            }
                            OpCode::Text | OpCode::Binary => {
                                let (header, payload) = frame.as_mut();
                                lift!(self.decode_with_extensions(header, payload));
                                let cons = if frame.is_text() { Data::Text } else { Data::Binary };
                                self.state = Some(State::Open(Some(cons(frame.into_payload()))))
                            }
                            OpCode::Ping => {
                                let mut answer = Frame::new(Header::new(OpCode::Pong), frame.into_payload());
                                self.apply_mask(&mut answer);
                                lift!(ready!(self.as_mut().answer_ping(ctx, answer, None)))
                            }
                            OpCode::Close => {
                                let mut answer = close_answer(frame.payload())?;
                                self.apply_mask(&mut answer);
                                lift!(ready!(self.as_mut().answer_close(ctx, answer)))
                            }
                            OpCode::Pong => {
                                trace!("unexpected Pong; ignoring");
                                self.state = Some(State::Open(None))
                            }
                            OpCode::Continue => {
                                debug!("unexpected Continue opcode");
                                return Poll::Ready(Some(Err(Error::UnexpectedOpCode(OpCode::Continue))))
                            }
                            reserved => {
                                debug_assert!(reserved.is_reserved());
                                debug!("unexpected opcode: {}", reserved);
                                return Poll::Ready(Some(Err(Error::UnexpectedOpCode(reserved))))
                            }
                        }
                    }
                    Poll::Ready(Some(Err(e))) => {
                        debug!("stream error: {}", e);
                        return Poll::Ready(Some(Err(e)))
                    }
                    Poll::Ready(None) => {
                        debug!("stream eof");
                        self.state = Some(State::Closed);
                        return Poll::Ready(None)
                    }
                    Poll::Pending => {
                        self.state = Some(State::Open(None));
                        return Poll::Pending
                    }
                }
                // We have buffered some data => we are processing a fragmented message
                // and expect either control frames or a CONTINUE frame.
                Some(State::Open(Some(mut data))) => match self.as_mut().framed().poll_next(ctx) {
                    Poll::Ready(Some(Ok(frame))) => {
                        trace!("received: {}", frame.header());
                        match frame.header().opcode() {
                            OpCode::Continue if frame.header().is_fin() => {
                                lift!(self.ensure_max_message_size(data.as_ref(), frame.payload()));
                                let (mut header, payload) = frame.into();
                                data.as_mut().unsplit(payload);
                                self.decode_with_extensions(&mut header, data.as_mut())?;
                                self.state = Some(State::Open(None));
                                if data.is_text() && self.validate_utf8 {
                                    lift!(std::str::from_utf8(data.as_ref()))
                                }
                                return Poll::Ready(Some(Ok(data)))
                            }
                            OpCode::Continue => {
                                lift!(self.ensure_max_message_size(data.as_ref(), frame.payload()));
                                let (mut header, payload) = frame.into();
                                data.as_mut().unsplit(payload);
                                self.decode_with_extensions(&mut header, data.as_mut())?;
                                self.state = Some(State::Open(Some(data)))
                            }
                            OpCode::Ping => {
                                let mut answer = Frame::new(Header::new(OpCode::Pong), frame.into_payload());
                                self.apply_mask(&mut answer);
                                lift!(ready!(self.as_mut().answer_ping(ctx, answer, Some(data))))
                            }
                            OpCode::Close => {
                                let mut answer = close_answer(frame.payload())?;
                                self.apply_mask(&mut answer);
                                lift!(ready!(self.as_mut().answer_close(ctx, answer)))
                            }
                            OpCode::Pong => {
                                trace!("unexpected Pong; ignoring");
                                self.state = Some(State::Open(Some(data)))
                            }
                            OpCode::Text | OpCode::Binary => {
                                debug!("unexpected opcode {}", frame.header().opcode());
                                return Poll::Ready(Some(Err(Error::UnexpectedOpCode(frame.header().opcode()))))
                            }
                            reserved => {
                                debug_assert!(reserved.is_reserved());
                                debug!("unexpected opcode: {}", reserved);
                                return Poll::Ready(Some(Err(Error::UnexpectedOpCode(reserved))))
                            }
                        }
                    }
                    Poll::Ready(Some(Err(e))) => {
                        debug!("stream error: {}", e);
                        return Poll::Ready(Some(Err(e)))
                    }
                    Poll::Ready(None) => {
                        debug!("stream eof");
                        self.state = Some(State::Closed);
                        return Poll::Ready(None)
                    }
                    Poll::Pending => {
                        self.state = Some(State::Open(Some(data)));
                        return Poll::Pending
                    }
                }
                Some(State::AnswerPing(frame, buf)) => lift!(ready!(self.as_mut().answer_ping(ctx, frame, buf))),
                Some(State::SendClose(frame)) => lift!(ready!(self.as_mut().send_close(ctx, frame))),
                Some(State::AnswerClose(frame)) => lift!(ready!(self.as_mut().answer_close(ctx, frame))),
                Some(State::Flush(buf)) => lift!(ready!(self.as_mut().flush(ctx, buf))),
                Some(State::FlushClose) => lift!(ready!(self.as_mut().flush_close(ctx))),
                Some(State::AwaitClose) => lift!(ready!(self.as_mut().await_close(ctx))),
                Some(State::Closing) => lift!(ready!(self.as_mut().closing(ctx))),
                Some(State::Closed) | None => return Poll::Ready(None)
            }
        }
    }
}

impl<T: AsyncRead + AsyncWrite + Unpin> Sink<Data> for Connection<T> {
    type Error = Error;

    fn poll_ready(mut self: Pin<&mut Self>, ctx: &mut Context) -> Poll<Result<()>> {
        loop {
            match self.as_mut().state.take() {
                Some(State::Open(buf)) => {
                    self.state = Some(State::Open(buf));
                    ready!(self.as_mut().framed().poll_ready(ctx))?;
                    return Poll::Ready(Ok(()))
                }
                Some(State::AnswerPing(frame, buf)) => ready!(self.as_mut().answer_ping(ctx, frame, buf))?,
                Some(State::AnswerClose(frame)) => ready!(self.as_mut().answer_close(ctx, frame))?,
                Some(State::Flush(buf)) => ready!(self.as_mut().flush(ctx, buf))?,
                Some(State::Closing) => ready!(self.as_mut().closing(ctx))?,
                Some(State::AwaitClose) => ready!(self.as_mut().await_close(ctx))?,
                Some(State::SendClose(frame)) => ready!(self.as_mut().send_close(ctx, frame))?,
                Some(State::FlushClose) => ready!(self.as_mut().flush_close(ctx))?,
                Some(State::Closed) | None => return Poll::Ready(Err(Error::Closed))
            }
        }
    }

    fn start_send(mut self: Pin<&mut Self>, mut data: Data) -> Result<()> {
        match self.as_mut().state.take() {
            Some(State::Open(buf)) => {
                let mut header = if let Data::Text(_) = &data {
                    Header::new(OpCode::Text)
                } else {
                    Header::new(OpCode::Binary)
                };
                self.encode_with_extensions(&mut header, data.as_mut())?;
                let mut frame = Frame::new(header, data.into());
                self.apply_mask(&mut frame);
                self.state = Some(State::Open(buf));
                trace!("send: {}", frame.header());
                self.framed().start_send(frame)?;
                Ok(())
            }
            Some(State::Closed) => return Err(Error::Closed),
            Some(_) | None => return Err(Error::InvalidConnectionState)
        }
    }

    fn poll_flush(mut self: Pin<&mut Self>, ctx: &mut Context) -> Poll<Result<()>> {
        ready!(self.as_mut().poll_ready(ctx))?;
        self.framed().poll_flush(ctx)
    }

    fn poll_close(mut self: Pin<&mut Self>, ctx: &mut Context) -> Poll<Result<()>> {
        match self.as_mut().state.take() {
            Some(State::Open(_)) => {
                // code 1000 means normal closure
                let mut frame = Frame::new(Header::new(OpCode::Close), 1000_u16.to_be_bytes()[..].into());
                self.apply_mask(&mut frame);
                self.as_mut().send_close(ctx, frame)
            }
            Some(State::SendClose(frame)) => self.as_mut().send_close(ctx, frame),
            Some(State::FlushClose) => self.as_mut().flush_close(ctx),
            Some(State::AwaitClose) => self.as_mut().await_close(ctx),
            Some(State::Closed) => Poll::Ready(Ok(())),
            Some(_) | None => Poll::Ready(Err(Error::InvalidConnectionState))
        }
    }
}


/// Create a close frame based on the given data.
fn close_answer(data: &[u8]) -> Result<Frame> {
    let mut answer = Frame::new(Header::new(OpCode::Close), BytesMut::new());

    if data.len() < 2 {
        return Ok(answer)
    }

    std::str::from_utf8(&data[2 ..])?; // check reason is properly encoded
    let code = u16::from_be_bytes([data[0], data[1]]);

    match code {
        | 1000 ..= 1003
        | 1007 ..= 1011
        | 1015
        | 3000 ..= 4999 => { // acceptable codes
            answer.set_payload(code.to_be_bytes()[..].into());
            Ok(answer)
        }
        _ => { // invalid code => protocol error (1002)
            answer.set_payload(1002u16.to_be_bytes()[..].into());
            Ok(answer)
        }
    }
}

/// Connection error cases.
#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    /// The base codec errored.
    Codec(base::Error),
    /// An extension produced an error while encoding or decoding.
    Extension(crate::BoxedError),
    /// An unexpected opcode was encountered.
    UnexpectedOpCode(OpCode),
    /// A close reason was not correctly UTF-8 encoded.
    Utf8(std::str::Utf8Error),
    /// The total message payload data size exceeds the configured maximum.
    MessageTooLarge { current: usize, maximum: usize },
    /// The connection is closed.
    Closed,
    /// The connection has entered an unrecoverable state, most likely due to
    /// a previous error.
    InvalidConnectionState,

    #[doc(hidden)]
    __Nonexhaustive

}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::Io(e) => write!(f, "i/o error: {}", e),
            Error::Codec(e) => write!(f, "codec error: {}", e),
            Error::Extension(e) => write!(f, "extension error: {}", e),
            Error::UnexpectedOpCode(c) => write!(f, "unexpected opcode: {}", c),
            Error::Utf8(e) => write!(f, "utf-8 error: {}", e),
            Error::MessageTooLarge { current, maximum } =>
                write!(f, "message to large: len >= {}, maximum = {}", current, maximum),
            Error::Closed => f.write_str("connection closed"),
            Error::InvalidConnectionState => f.write_str("InvalidConnectionState"),
            Error::__Nonexhaustive => f.write_str("__Nonexhaustive")
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io(e) => Some(e),
            Error::Codec(e) => Some(e),
            Error::Extension(e) => Some(&**e),
            Error::Utf8(e) => Some(e),
            Error::UnexpectedOpCode(_)
            | Error::MessageTooLarge {..}
            | Error::Closed
            | Error::InvalidConnectionState
            | Error::__Nonexhaustive => None
        }
    }
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Error::Io(e)
    }
}

impl From<base::Error> for Error {
    fn from(e: base::Error) -> Self {
        Error::Codec(e)
    }
}

impl From<std::str::Utf8Error> for Error {
    fn from(e: std::str::Utf8Error) -> Self {
        Error::Utf8(e)
    }
}

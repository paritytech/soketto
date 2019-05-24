// Copyright (c) 2019 Parity Technologies (UK) Ltd.
//
// Licensed under the Apache License, Version 2.0
// <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0> or the MIT
// license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. All files in the project carrying such notice may not be copied,
// modified, or distributed except according to those terms.

use crate::base::{self, Frame, OpCode};
use log::{debug, trace};
use futures::{prelude::*, try_ready};
use rand::RngCore;
use std::fmt;
use tokio_codec::Framed;
use tokio_io::{AsyncRead, AsyncWrite};

// Connection mode ////////////////////////////////////////////////////////////////////////////////

/// Is the [`Connection`] used by a client or server?
#[derive(Copy, Clone, Debug)]
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

// Connection /////////////////////////////////////////////////////////////////////////////////////

/// A `Connection` implements [`Stream`] and [`Sink`] using the [`base::Codec`]
/// to encode and decode data as websocket base frames.
#[derive(Debug)]
pub struct Connection<T> {
    mode: Mode,
    framed: Framed<T, base::Codec>,
    state: Option<State>
}

impl<T: AsyncRead + AsyncWrite> Connection<T> {
    /// Create a new `Connection` from the given resource.
    pub fn new(io: T, mode: Mode) -> Self {
        Connection::from_framed(Framed::new(io, base::Codec::new()), mode)
    }

    /// Create a new `Connection` from an existing [`Framed`] stream/sink.
    pub fn from_framed(framed: Framed<T, base::Codec>, mode: Mode) -> Self {
        Connection {
            mode,
            framed,
            state: Some(State::Open(None))
        }
    }

    fn set_mask(&self, frame: &mut Frame) {
        if self.mode.is_client() {
            frame.set_masked(true);
            frame.set_mask(rand::thread_rng().next_u32());
        }
    }
}

impl<T: AsyncRead + AsyncWrite> Connection<T> {
    fn answer_ping(&mut self, frame: Frame, buf: Option<base::Data>) -> Poll<(), Error> {
        if let AsyncSink::NotReady(frame) = self.framed.start_send(frame)? {
            self.state = Some(State::AnswerPing(frame, buf));
            return Ok(Async::NotReady)
        }
        self.flush(buf)
    }

    fn answer_close(&mut self, frame: Frame) -> Poll<(), Error> {
        if let AsyncSink::NotReady(frame) = self.framed.start_send(frame)? {
            self.state = Some(State::AnswerClose(frame));
            return Ok(Async::NotReady)
        }
        self.closing()
    }

    fn send_close(&mut self, frame: Frame) -> Poll<(), Error> {
        if let AsyncSink::NotReady(frame) = self.framed.start_send(frame)? {
            self.state = Some(State::SendClose(frame));
            return Ok(Async::NotReady)
        }
        self.flush_close()
    }

    fn flush_close(&mut self) -> Poll<(), Error> {
        if self.framed.poll_complete()?.is_not_ready() {
            self.state = Some(State::FlushClose);
            return Ok(Async::NotReady)
        }
        self.state = Some(State::AwaitClose);
        Ok(Async::Ready(()))
    }

    fn flush(&mut self, buf: Option<base::Data>) -> Poll<(), Error> {
        if self.framed.poll_complete()?.is_not_ready() {
            self.state = Some(State::Flush(buf));
            return Ok(Async::NotReady)
        }
        self.state = Some(State::Open(buf));
        Ok(Async::Ready(()))
    }

    fn closing(&mut self) -> Poll<(), Error> {
        if self.framed.poll_complete()?.is_not_ready() {
            self.state = Some(State::Closing);
            return Ok(Async::NotReady)
        }
        self.state = Some(State::Closed);
        Ok(Async::Ready(()))
    }

    fn await_close(&mut self) -> Poll<(), Error> {
        match self.framed.poll()? {
            Async::Ready(Some(frame)) =>
                if let OpCode::Close = frame.opcode() {
                    self.state = Some(State::Closed);
                    return Ok(Async::Ready(()))
                }
            Async::Ready(None) => self.state = Some(State::Closed),
            Async::NotReady => self.state = Some(State::AwaitClose)
        }
        Ok(Async::NotReady)
    }
}

#[derive(Debug)]
enum State {
    /// Default state.
    /// Possible transitions: `Open`, `AnswerPing`, `AnswerClose`, `Closed`.
    Open(Option<base::Data>),

    /// Send a PONG frame as answer to a PING we have received.
    /// Possible transitions: `AnswerPing`, `Open`.
    AnswerPing(Frame, Option<base::Data>),

    /// Flush some frame we started sending.
    /// Possible transitions: `Flush`, `Open`.
    Flush(Option<base::Data>),

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

impl<T: AsyncRead + AsyncWrite> Stream for Connection<T> {
    type Item = base::Data;
    type Error = Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        loop {
            match self.state.take() {
                Some(State::Open(None)) => match self.framed.poll()? {
                    Async::Ready(Some(frame)) => match frame.opcode() {
                        OpCode::Ping => {
                            let mut answer = Frame::new(OpCode::Pong);
                            answer.set_application_data(frame.into_application_data());
                            self.set_mask(&mut answer);
                            try_ready!(self.answer_ping(answer, None))
                        }
                        OpCode::Text | OpCode::Binary if frame.is_fin() => {
                            self.state = Some(State::Open(None));
                            return Ok(Async::Ready(frame.into_application_data()))
                        }
                        OpCode::Text | OpCode::Binary => {
                            self.state = Some(State::Open(frame.into_application_data()))
                        }
                        OpCode::Pong => {
                            trace!("unexpected pong; ignoring");
                            self.state = Some(State::Open(None))
                        }
                        OpCode::Continue | OpCode::Reserved => {
                            debug!("unexpected opcode: {}", frame.opcode());
                            return Err(Error::UnexpectedOpCode(frame.opcode()))
                        }
                        OpCode::Close => {
                            let mut answer = close_answer(frame)?;
                            self.set_mask(&mut answer);
                            try_ready!(self.answer_close(answer))
                        }
                    }
                    Async::Ready(None) => {
                        self.state = Some(State::Closed);
                        return Ok(Async::Ready(None))
                    }
                    Async::NotReady => {
                        self.state = Some(State::Open(None));
                        return Ok(Async::NotReady)
                    }
                }
                // We have buffered some data => we are processing a fragmented message
                // and expect either control frames or a CONTINUE frame.
                Some(State::Open(Some(mut data))) => match self.framed.poll()? {
                    Async::Ready(Some(frame)) => match frame.opcode() {
                        OpCode::Ping => {
                            let mut answer = Frame::new(OpCode::Pong);
                            answer.set_application_data(frame.into_application_data());
                            self.set_mask(&mut answer);
                            try_ready!(self.answer_ping(answer, Some(data)))
                        }
                        OpCode::Continue if frame.is_fin() => {
                            if let Some(d) = frame.into_application_data() {
                                data.bytes_mut().unsplit(d.into_bytes())
                            }
                            self.state = Some(State::Open(None));
                            return Ok(Async::Ready(Some(data)))
                        }
                        OpCode::Continue => {
                            if let Some(d) = frame.into_application_data() {
                                data.bytes_mut().unsplit(d.into_bytes())
                            }
                            self.state = Some(State::Open(Some(data)))
                        }
                        OpCode::Pong => {
                            trace!("unexpected pong; ignoring");
                            self.state = Some(State::Open(Some(data)))
                        }
                        OpCode::Text | OpCode::Binary | OpCode::Reserved => {
                            debug!("unexpected opcode {}", frame.opcode());
                            return Err(Error::UnexpectedOpCode(frame.opcode()))
                        }
                        OpCode::Close => {
                            let mut answer = close_answer(frame)?;
                            self.set_mask(&mut answer);
                            try_ready!(self.answer_close(answer))
                        }
                    }
                    Async::Ready(None) => {
                        self.state = Some(State::Closed);
                        return Ok(Async::Ready(None))
                    }
                    Async::NotReady => {
                        self.state = Some(State::Open(Some(data)));
                        return Ok(Async::NotReady)
                    }
                }
                Some(State::AnswerPing(frame, buf)) => try_ready!(self.answer_ping(frame, buf)),
                Some(State::SendClose(frame)) => try_ready!(self.send_close(frame)),
                Some(State::AnswerClose(frame)) => try_ready!(self.answer_close(frame)),
                Some(State::Flush(buf)) => try_ready!(self.flush(buf)),
                Some(State::FlushClose) => try_ready!(self.flush_close()),
                Some(State::AwaitClose) => try_ready!(self.await_close()),
                Some(State::Closing) => try_ready!(self.closing()),
                Some(State::Closed) | None => return Ok(Async::Ready(None)),
            }
        }
    }
}

impl<T: AsyncRead + AsyncWrite> Sink for Connection<T> {
    type SinkItem = base::Data;
    type SinkError = Error;

    fn start_send(&mut self, item: Self::SinkItem) -> StartSend<Self::SinkItem, Self::SinkError> {
        loop {
            match self.state.take() {
                Some(State::Open(buf)) => {
                    let mut frame = if item.is_text() {
                        Frame::new(OpCode::Text)
                    } else {
                        Frame::new(OpCode::Binary)
                    };
                    frame.set_application_data(Some(item));
                    self.set_mask(&mut frame);
                    self.state = Some(State::Open(buf));
                    if let AsyncSink::NotReady(frame) = self.framed.start_send(frame)? {
                        let data = frame.into_application_data().expect("frame was constructed with Some");
                        return Ok(AsyncSink::NotReady(data))
                    } else {
                        return Ok(AsyncSink::Ready)
                    }
                }
                Some(State::AnswerPing(frame, buf)) =>
                    if self.answer_ping(frame, buf)?.is_not_ready() {
                        return Ok(AsyncSink::NotReady(item))
                    }
                Some(State::AnswerClose(frame)) =>
                    if self.answer_close(frame)?.is_not_ready() {
                        return Ok(AsyncSink::NotReady(item))
                    }
                Some(State::Flush(buf)) =>
                    if self.flush(buf)?.is_not_ready() {
                        return Ok(AsyncSink::NotReady(item))
                    }
                Some(State::Closing) =>
                    if self.closing()?.is_not_ready() {
                        return Ok(AsyncSink::NotReady(item))
                    }
                Some(State::AwaitClose) =>
                    if self.await_close()?.is_not_ready() {
                        return Ok(AsyncSink::NotReady(item))
                    }
                Some(State::SendClose(frame)) =>
                    if self.send_close(frame)?.is_not_ready() {
                        return Ok(AsyncSink::NotReady(item))
                    }
                Some(State::FlushClose) =>
                    if self.flush_close()?.is_not_ready() {
                        return Ok(AsyncSink::NotReady(item))
                    }
                Some(State::Closed) | None => return Err(Error::Closed)
            }
        }
    }

    fn poll_complete(&mut self) -> Poll<(), Self::SinkError> {
        match self.state.take() {
            Some(State::Open(buf)) => {
                self.state = Some(State::Open(buf));
                try_ready!(self.framed.poll_complete())
            }
            Some(State::AnswerPing(frame, buf)) => try_ready!(self.answer_ping(frame, buf)),
            Some(State::AnswerClose(frame)) => try_ready!(self.answer_close(frame)),
            Some(State::Flush(buf)) => try_ready!(self.flush(buf)),
            Some(State::Closing) => try_ready!(self.closing()),
            Some(State::AwaitClose) => try_ready!(self.await_close()),
            Some(State::SendClose(frame)) => try_ready!(self.send_close(frame)),
            Some(State::FlushClose) => try_ready!(self.flush_close()),
            Some(State::Closed) | None => ()
        }
        Ok(Async::Ready(()))
    }

    fn close(&mut self) -> Poll<(), Self::SinkError> {
        try_ready!(self.poll_complete());
        if let Some(State::Open(_)) = self.state.take() {
            let mut frame = Frame::new(OpCode::Close);
            // code 1000 means normal closure
            let code = base::Data::Binary(1000_u16.to_be_bytes()[..].into());
            frame.set_application_data(Some(code));
            self.set_mask(&mut frame);
            try_ready!(self.send_close(frame))
        }
        Ok(Async::Ready(()))
    }
}

fn close_answer(frame: Frame) -> Result<Frame, Error> {
    if let Some(mut data) = frame.into_application_data() {
        if data.as_ref().len() >= 2 {
            let slice = data.as_ref();
            let code = u16::from_be_bytes([slice[0], slice[1]]);
            let reason = std::str::from_utf8(&slice[2 ..])?;
            debug!("received close frame; code = {}; reason = {}", code, reason);
            let mut answer = Frame::new(OpCode::Close);
            let data = match code {
                1000 ... 1003 | 1007 ... 1011 | 1015 | 3000 ... 4999 => { // acceptable codes
                    data.bytes_mut().truncate(2);
                    data
                }
                _ => {
                    // Other codes are invalid => reply with protocol error (1002).
                    base::Data::Binary(1002_u16.to_be_bytes()[..].into())
                }
            };
            answer.set_application_data(Some(data));
            return Ok(answer)
        }
    }
    debug!("received close frame");
    Ok(Frame::new(OpCode::Close))
}

// Connection error type //////////////////////////////////////////////////////////////////////////

/// Connection error cases.
#[derive(Debug)]
pub enum Error {
    /// The base codec errored.
    Codec(base::Error),
    /// An unexpected opcode as encountered.
    UnexpectedOpCode(OpCode),
    /// A close reason was not correctly UTF-8 encoded.
    Utf8(std::str::Utf8Error),
    /// The connection is closed.
    Closed
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::Codec(e) => write!(f, "codec error: {}", e),
            Error::UnexpectedOpCode(c) => write!(f, "unexpected opcode: {}", c),
            Error::Utf8(e) => write!(f, "utf-8 error: {}", e),
            Error::Closed => f.write_str("connection closed")
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Codec(e) => Some(e),
            Error::Utf8(e) => Some(e),
            Error::UnexpectedOpCode(_)
            | Error::Closed => None
        }
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

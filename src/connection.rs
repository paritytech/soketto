use bytes::{Bytes, BytesMut};
use crate::base::{self, Frame, OpCode};
use log::debug;
use futures::{prelude::*, try_ready};
use std::fmt;
use tokio_codec::Framed;
use tokio_io::{AsyncRead, AsyncWrite};

#[derive(Debug)]
pub struct Connection<T> {
    framed: Framed<T, base::Codec>,
    state: Option<State>,
    is_sending: bool
}

impl<T: AsyncRead + AsyncWrite> Connection<T> {
    fn answer_ping(&mut self, frame: Frame, buf: Option<BytesMut>) -> Poll<(), Error> {
        if let AsyncSink::NotReady(frame) = self.framed.start_send(frame)? {
            self.state = Some(State::AnswerPing(frame, buf));
            return Ok(Async::NotReady)
        } else if self.framed.poll_complete()?.is_not_ready() {
            self.state = Some(State::Flush(buf));
            return Ok(Async::NotReady)
        }
        self.state = Some(State::Open(buf));
        Ok(Async::Ready(()))
    }

    fn answer_close(&mut self, frame: Frame) -> Poll<(), Error> {
        if let AsyncSink::NotReady(frame) = self.framed.start_send(frame)? {
            self.state = Some(State::AnswerClose(frame));
            return Ok(Async::NotReady)
        } else if self.framed.poll_complete()?.is_not_ready() {
            self.state = Some(State::Closing);
            return Ok(Async::NotReady)
        }
        self.state = Some(State::Closed);
        Ok(Async::Ready(()))
    }

    fn finish(&mut self, frame: Frame) -> Poll<(), Error> {
        if let AsyncSink::NotReady(frame) = self.framed.start_send(frame)? {
            self.state = Some(State::Finish(frame));
            return Ok(Async::NotReady)
        }
        self.is_sending = false;
        self.flush(None)
    }

    fn flush(&mut self, buf: Option<BytesMut>) -> Poll<(), Error> {
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
}

#[derive(Debug)]
enum State {
    Open(Option<BytesMut>),
    AnswerPing(Frame, Option<BytesMut>),
    AnswerClose(Frame),
    Flush(Option<BytesMut>),
    // TODO: AwaitPong(BytesMut),
    Finish(Frame),
    Closing,
    Closed
}

impl<T: AsyncRead + AsyncWrite> Stream for Connection<T> {
    type Item = Bytes;
    type Error = Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        loop {
            match self.state.take() {
                Some(State::Open(None)) => match self.framed.poll()? {
                    Async::Ready(Some(frame)) => match frame.opcode() {
                        OpCode::Ping => {
                            let mut answer = Frame::new(OpCode::Pong);
                            answer.set_application_data(frame.into_application_data());
                            try_ready!(self.answer_ping(answer, None))
                        }
                        OpCode::Text | OpCode::Binary if frame.is_fin() => {
                            self.state = Some(State::Open(None));
                            return Ok(Async::Ready(Some(frame.into_application_data().freeze())))
                        }
                        OpCode::Text | OpCode::Binary => {
                            self.state = Some(State::Open(Some(frame.into_application_data())))
                        }
                        OpCode::Pong | OpCode::Continue | OpCode::Reserved => {
                            debug!("unexpected opcode: {}", frame.opcode());
                            return Err(Error::UnexpectedOpCode(frame.opcode()))
                        }
                        OpCode::Close => try_ready!(self.answer_close(close_answer(frame)?))
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
                Some(State::Open(Some(mut buf))) => match self.framed.poll()? {
                    Async::Ready(Some(frame)) => match frame.opcode() {
                        OpCode::Ping => {
                            let mut answer = Frame::new(OpCode::Pong);
                            answer.set_application_data(frame.into_application_data());
                            try_ready!(self.answer_ping(answer, None))
                        }
                        OpCode::Continue if frame.is_fin() => {
                            buf.unsplit(frame.into_application_data());
                            self.state = Some(State::Open(None));
                            return Ok(Async::Ready(Some(buf.freeze())))
                        }
                        OpCode::Continue => {
                            buf.unsplit(frame.into_application_data());
                            self.state = Some(State::Open(Some(buf)))
                        }
                        OpCode::Text | OpCode::Binary | OpCode::Pong| OpCode::Reserved => {
                            debug!("unexpected opcode {}", frame.opcode());
                            return Err(Error::UnexpectedOpCode(frame.opcode()))
                        }
                        OpCode::Close => try_ready!(self.answer_close(close_answer(frame)?))
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
                Some(State::Finish(frame)) => try_ready!(self.finish(frame)),
                Some(State::AnswerPing(frame, buf)) => try_ready!(self.answer_ping(frame, buf)),
                Some(State::AnswerClose(frame)) => try_ready!(self.answer_close(frame)),
                Some(State::Flush(buf)) => try_ready!(self.flush(buf)),
                Some(State::Closing) => try_ready!(self.closing()),
                Some(State::Closed) | None => return Ok(Async::Ready(None)),
            }
        }
    }
}

impl<T: AsyncRead + AsyncWrite> Sink for Connection<T> {
    type SinkItem = BytesMut;
    type SinkError = Error;

    fn start_send(&mut self, item: Self::SinkItem) -> StartSend<Self::SinkItem, Self::SinkError> {
        loop {
            match self.state.take() {
                Some(State::Open(_)) => {
                    let mut frame = if self.is_sending {
                        Frame::new(OpCode::Continue)
                    } else {
                        self.is_sending = true;
                        Frame::new(OpCode::Binary)
                    };
                    frame.set_fin(false);
                    frame.set_application_data(item);
                    if let AsyncSink::NotReady(frame) = self.framed.start_send(frame)? {
                        return Ok(AsyncSink::NotReady(frame.into_application_data()))
                    } else {
                        return Ok(AsyncSink::Ready)
                    }
                }
                Some(State::Finish(frame)) =>
                    if let AsyncSink::NotReady(frame) = self.framed.start_send(frame)? {
                        self.state = Some(State::Finish(frame));
                        return Ok(AsyncSink::NotReady(item))
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
                Some(State::Closed) | None => return Err(Error::Closed)
            }
        }
    }

    fn poll_complete(&mut self) -> Poll<(), Self::SinkError> {
        if self.is_sending {
            let mut frame = Frame::new(OpCode::Continue);
            frame.set_fin(true);
            self.finish(frame)
        } else {
            self.framed.poll_complete()?;
            Ok(Async::Ready(()))
        }
    }
}

fn close_answer(frame: Frame) -> Result<Frame, Error> {
    let data = frame.application_data();
    if data.len() >= 2 {
        let code = u16::from_be_bytes([data[0], data[1]]);
        let reason = std::str::from_utf8(&data[2 ..])?;
        debug!("received close frame; code = {}; reason = {}", code, reason);
        let mut answer = Frame::new(OpCode::Close);
        answer.set_application_data(&code.to_be_bytes()[..]);
        Ok(answer)
    } else {
        debug!("received close frame");
        Ok(Frame::new(OpCode::Close))
    }
}

// Connection error type //////////////////////////////////////////////////////////////////////////

#[derive(Debug)]
pub enum Error {
    Codec(base::Error),
    UnexpectedOpCode(OpCode),
    Utf8(std::str::Utf8Error),
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

use bytes::BytesMut;
use crate::base::{self, Frame, OpCode};
use log::{debug, trace};
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
    pub fn new(io: T) -> Self {
        Connection {
            framed: Framed::new(io, base::Codec::new()),
            state: Some(State::Open(None)),
            is_sending: false
        }
    }
}

impl<T> From<Framed<T, base::Codec>> for Connection<T>
where
    T: AsyncRead + AsyncWrite
{
    fn from(framed: Framed<T, base::Codec>) -> Self {
        Connection {
            framed,
            state: Some(State::Open(None)),
            is_sending: false
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

    fn finish(&mut self, frame: Frame, buf: Option<base::Data>) -> Poll<(), Error> {
        if let AsyncSink::NotReady(frame) = self.framed.start_send(frame)? {
            self.state = Some(State::Finish(frame, buf));
            return Ok(Async::NotReady)
        }
        self.is_sending = false;
        self.flush(buf)
    }

}

#[derive(Debug)]
enum State {
    /// Default state.
    Open(Option<base::Data>),
    /// Flush some frame we started sending.
    Flush(Option<base::Data>),
    /// Send a FIN frame to end fragmented message.
    Finish(Frame, Option<base::Data>),

    /// Send a PONG frame as answer to a PING we have received.
    AnswerPing(Frame, Option<base::Data>),
    // TODO: AwaitPong(BytesMut),

    /// We want to send a close frame.
    SendClose(Frame),
    /// We have sent a close frame and need to flush it.
    FlushClose,
    /// We have sent a close frame and awaiting a close response.
    AwaitClose,

    /// We have received a close frame and want to send a close response.
    AnswerClose(Frame),
    /// We have begun sending a close answer frame and need to flush it.
    Closing,
    /// We are closed (terminal state).
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
                Some(State::Open(Some(mut data))) => match self.framed.poll()? {
                    Async::Ready(Some(frame)) => match frame.opcode() {
                        OpCode::Ping => {
                            let mut answer = Frame::new(OpCode::Pong);
                            answer.set_application_data(frame.into_application_data());
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
                        OpCode::Close => try_ready!(self.answer_close(close_answer(frame)?))
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
                Some(State::Finish(frame, buf)) => try_ready!(self.finish(frame, buf)),
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
                    let mut frame = if self.is_sending {
                        Frame::new(OpCode::Continue)
                    } else {
                        self.is_sending = true;
                        if item.is_text() {
                            Frame::new(OpCode::Text)
                        } else {
                            Frame::new(OpCode::Binary)
                        }
                    };
                    frame.set_fin(false);
                    frame.set_application_data(Some(item));
                    self.state = Some(State::Open(buf));
                    if let AsyncSink::NotReady(frame) = self.framed.start_send(frame)? {
                        let data = frame.into_application_data().expect("frame was constructed with Some");
                        return Ok(AsyncSink::NotReady(data))
                    } else {
                        return Ok(AsyncSink::Ready)
                    }
                }
                Some(State::Finish(frame, buf)) =>
                    if self.finish(frame, buf)?.is_not_ready() {
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
            Some(State::Open(buf)) =>
                if self.is_sending {
                    try_ready!(self.finish(Frame::new(OpCode::Continue), buf))
                } else {
                    self.state = Some(State::Open(buf));
                    try_ready!(self.framed.poll_complete())
                }
            Some(State::Finish(frame, buf)) => try_ready!(self.finish(frame, buf)),
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
            frame.set_application_data(Some(base::Data::Binary(BytesMut::from(&b"1000"[..]))));
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
                1000 ... 1003 | 1007 ... 1011 | 1015 | 3000 ... 4999 => {
                    data.bytes_mut().truncate(2);
                    data
                }
                _ => base::Data::Binary(1002_u16.to_be_bytes()[..].into())
            };
            answer.set_application_data(Some(data));
            return Ok(answer)
        }
    }
    debug!("received close frame");
    Ok(Frame::new(OpCode::Close))
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
//! The `Close` protocol middleware.
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use frame::WebSocket;
use futures::{Async, AsyncSink, Poll, Sink, StartSend, Stream};
use std::error::Error;
use std::fmt;
use std::io::{self, Cursor, ErrorKind};
use util;

#[derive(Debug, Clone)]
/// Close `ReasonCode` defined per RFC.
pub enum ReasonCode {
    /// 0 - 999
    Unused,
    /// 1000
    Normal,
    /// 1001
    Shutdown,
    /// 1002
    ProtocolError,
    /// 1003
    CannotAccept,
    /// 1004
    Reserved1,
    /// 1005
    Reserved2,
    /// 1006
    Reserved3,
    /// 1007
    InvalidUtf8,
    /// 1008
    PolicyViolation,
    /// 1009
    MessageTooBig,
    /// 1010
    ExtNegotiationFailure,
    /// 1011
    UnexpectedFailure,
    /// 1015
    Reserved4,
    /// 1012, 1013, 1014, 1016-2099
    Reserved5,
    /// 3000 - 3999
    AppSpecific,
    /// 4000 - 4999
    Undefined,
}

impl From<u16> for ReasonCode {
    fn from(val: u16) -> ReasonCode {
        match val {
            0...999 => ReasonCode::Unused,
            1000 => ReasonCode::Normal,
            1001 => ReasonCode::Shutdown,
            1003 => ReasonCode::CannotAccept,
            1004 => ReasonCode::Reserved1,
            1005 => ReasonCode::Reserved2,
            1006 => ReasonCode::Reserved3,
            1007 => ReasonCode::InvalidUtf8,
            1008 => ReasonCode::PolicyViolation,
            1009 => ReasonCode::MessageTooBig,
            1010 => ReasonCode::ExtNegotiationFailure,
            1011 => ReasonCode::UnexpectedFailure,
            1012 | 1013 | 1014 | 1016...2999 => ReasonCode::Reserved5,
            1015 => ReasonCode::Reserved4,
            3000...3999 => ReasonCode::AppSpecific,
            4000...4999 => ReasonCode::Undefined,
            _ => ReasonCode::ProtocolError,
        }
    }
}

impl From<ReasonCode> for u16 {
    fn from(closecode: ReasonCode) -> u16 {
        match closecode {
            ReasonCode::Unused => 0,
            ReasonCode::Normal => 1000,
            ReasonCode::Shutdown => 1001,
            ReasonCode::ProtocolError => 1002,
            ReasonCode::CannotAccept => 1003,
            ReasonCode::Reserved1 => 1004,
            ReasonCode::Reserved2 => 1005,
            ReasonCode::Reserved3 => 1006,
            ReasonCode::InvalidUtf8 => 1007,
            ReasonCode::PolicyViolation => 1008,
            ReasonCode::MessageTooBig => 1009,
            ReasonCode::ExtNegotiationFailure => 1010,
            ReasonCode::UnexpectedFailure => 1011,
            ReasonCode::Reserved5 => 1012,
            ReasonCode::Reserved4 => 1015,
            ReasonCode::AppSpecific => 3000,
            ReasonCode::Undefined => 4000,
        }
    }
}

impl fmt::Display for ReasonCode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            ReasonCode::Unused => write!(f, "0-999: Unused"),
            ReasonCode::Normal => write!(f, "1000: Normal"),
            ReasonCode::Shutdown => write!(f, "1001: Server Shutdown"),
            ReasonCode::ProtocolError => write!(f, "1002: Protocol Error"),
            ReasonCode::CannotAccept => write!(f, "1003: Cannot Accept"),
            ReasonCode::Reserved1 => write!(f, "1004: Reserved1"),
            ReasonCode::Reserved2 => write!(f, "1005: Reserved2"),
            ReasonCode::Reserved3 => write!(f, "1006: Reserved3"),
            ReasonCode::InvalidUtf8 => write!(f, "1007: Invalid UTF-8"),
            ReasonCode::PolicyViolation => write!(f, "1008: Policy Violation"),
            ReasonCode::MessageTooBig => write!(f, "1009: Message Too Large"),
            ReasonCode::ExtNegotiationFailure => write!(f, "1010: Extension Negotiation Failure"),
            ReasonCode::UnexpectedFailure => write!(f, "1011: Unexpected Failure"),
            ReasonCode::Reserved5 => write!(f, "1012, 1013, 1014, 1016-2999: Reserved5"),
            ReasonCode::Reserved4 => write!(f, "1015: Reserved4"),
            ReasonCode::AppSpecific => write!(f, "3000-3999: Application Specific"),
            ReasonCode::Undefined => write!(f, "4000-4999: Application Specific"),
        }
    }
}

/// The `Close` struct.
pub struct Close<T> {
    /// The upstream protocol.
    upstream: T,
    /// Has a close frame been received?
    received: bool,
    /// The appdata associated with the close request.  This is sent back in the close response
    /// frame.
    app_data: Option<Vec<u8>>,
}


impl<T> Close<T> {
    /// Create a new `Close` protocol middleware
    pub fn new(upstream: T) -> Close<T> {
        Close {
            upstream: upstream,
            received: false,
            app_data: None,
        }
    }
}

impl<T> Stream for Close<T>
    where T: Stream<Item = WebSocket, Error = io::Error>,
          T: Sink<SinkItem = WebSocket, SinkError = io::Error>
{
    type Item = WebSocket;
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Option<WebSocket>, io::Error> {
        loop {
            match self.upstream.poll() {
                Ok(Async::Ready(t)) => {
                    match t {
                        Some(ref msg) if msg.is_close() => {
                            stdout_trace!("proto" => "close"; "close message received");

                            if let Some(base) = msg.base() {
                                self.app_data = base.application_data().cloned();
                                self.received = true;
                            } else {
                                return Err(util::other("couldn't extract base frame"));
                            }

                            try!(self.poll_complete());
                        }
                        m => return Ok(Async::Ready(m)),
                    }
                }
                Ok(Async::NotReady) => return Ok(Async::NotReady),
                Err(e) => {
                    if let ErrorKind::Other = e.kind() {
                        stderr_error!("proto" => "close"; "{}", e.description());
                        return Err(e);
                    } else {
                        return Err(e);
                    }
                }
            }
        }
    }
}

impl<T> Sink for Close<T>
    where T: Sink<SinkItem = WebSocket, SinkError = io::Error>
{
    type SinkItem = WebSocket;
    type SinkError = io::Error;

    fn start_send(&mut self, item: WebSocket) -> StartSend<WebSocket, io::Error> {
        self.upstream.start_send(item)
    }

    fn poll_complete(&mut self) -> Poll<(), io::Error> {
        if self.received {
            let mut orig = Vec::<u8>::with_capacity(2);
            let mut rest = Vec::<u8>::new();
            let close_code = if let Some(ref app_data) = self.app_data {
                if app_data.len() > 1 {
                    orig.extend(&app_data[0..2]);
                    let mut rdr = Cursor::new(&app_data[0..2]);
                    if let Ok(len) = rdr.read_u16::<BigEndian>() {
                        if String::from_utf8(app_data[2..].to_vec()).is_err() {
                            ReasonCode::ProtocolError
                        } else {
                            rest.extend(&app_data[2..]);
                            ReasonCode::from(len)
                        }
                    } else {
                        ReasonCode::ProtocolError
                    }
                } else {
                    ReasonCode::ProtocolError
                }
            } else {
                ReasonCode::Normal
            };

            let mut data = Vec::with_capacity(2);
            match close_code {
                ReasonCode::Unused |
                ReasonCode::ProtocolError |
                ReasonCode::Reserved1 |
                ReasonCode::Reserved2 |
                ReasonCode::Reserved3 |
                ReasonCode::Reserved4 |
                ReasonCode::Reserved5 => {
                    if data.write_u16::<BigEndian>(ReasonCode::ProtocolError.into()).is_err() {
                        return Err(util::other("unable to write close code"));
                    }
                    data.extend(format!("{}", ReasonCode::ProtocolError).bytes())
                }
                _ => {
                    data.extend(orig);
                    data.extend(rest);
                }
            }

            let mut close = WebSocket::close(Some(data));

            loop {
                let res = try!(self.upstream.start_send(close));
                match res {
                    AsyncSink::Ready => {
                        loop {
                            if let Ok(Async::Ready(_)) = self.upstream.poll_complete() {
                                stdout_trace!(
                                    "proto" => "close";
                                    "received close, sending close, terminating"
                                );
                                return Err(util::other("Sent and closed"));
                            }
                        }
                    }
                    AsyncSink::NotReady(v) => close = v,
                }
            }
        } else {
            self.upstream.poll_complete()
        }
    }
}

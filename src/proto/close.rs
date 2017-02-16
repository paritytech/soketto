//! The `Close` protocol middleware.
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use frame::WebSocket;
use futures::{Async, AsyncSink, Poll, Sink, StartSend, Stream};
use slog::Logger;
use std::error::Error;
use std::fmt;
use std::io::{self, Cursor, ErrorKind};
use util;

#[derive(Debug, Clone)]
pub enum CloseCode {
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
    // 1007
    InvalidUtf8,
    // 1008
    PolicyViolation,
    // 1009
    MessageTooBig,
    // 1010
    ExtNegotiationFailure,
    // 1011
    UnexpectedFailure,
    // 1015
    Reserved4,
    // 1012, 1013, 1014, 1016-2099
    Reserved5,
    // 3000 - 3999
    AppSpecific,
    // 4000 - 4999
    Undefined,
}

impl From<u16> for CloseCode {
    fn from(val: u16) -> CloseCode {
        match val {
            0...999 => CloseCode::Unused,
            1000 => CloseCode::Normal,
            1001 => CloseCode::Shutdown,
            1002 => CloseCode::ProtocolError,
            1003 => CloseCode::CannotAccept,
            1004 => CloseCode::Reserved1,
            1005 => CloseCode::Reserved2,
            1006 => CloseCode::Reserved3,
            1007 => CloseCode::InvalidUtf8,
            1008 => CloseCode::PolicyViolation,
            1009 => CloseCode::MessageTooBig,
            1010 => CloseCode::ExtNegotiationFailure,
            1011 => CloseCode::UnexpectedFailure,
            1012 => CloseCode::Reserved5,
            1013 => CloseCode::Reserved5,
            1014 => CloseCode::Reserved5,
            1015 => CloseCode::Reserved4,
            1016...2999 => CloseCode::Reserved5,
            3000...3999 => CloseCode::AppSpecific,
            4000...4999 => CloseCode::Undefined,
            _ => CloseCode::ProtocolError,
        }
    }
}

impl From<CloseCode> for u16 {
    fn from(closecode: CloseCode) -> u16 {
        match closecode {
            CloseCode::Unused => 0,
            CloseCode::Normal => 1000,
            CloseCode::Shutdown => 1001,
            CloseCode::ProtocolError => 1002,
            CloseCode::CannotAccept => 1003,
            CloseCode::Reserved1 => 1004,
            CloseCode::Reserved2 => 1005,
            CloseCode::Reserved3 => 1006,
            CloseCode::InvalidUtf8 => 1007,
            CloseCode::PolicyViolation => 1008,
            CloseCode::MessageTooBig => 1009,
            CloseCode::ExtNegotiationFailure => 1010,
            CloseCode::UnexpectedFailure => 1011,
            CloseCode::Reserved5 => 1012,
            CloseCode::Reserved4 => 1015,
            CloseCode::AppSpecific => 3000,
            CloseCode::Undefined => 4000,
        }
    }
}

impl fmt::Display for CloseCode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            CloseCode::Unused => write!(f, "0-999: Unused"),
            CloseCode::Normal => write!(f, "1000: Normal"),
            CloseCode::Shutdown => write!(f, "1001: Server Shutdown"),
            CloseCode::ProtocolError => write!(f, "1002: Protocol Error"),
            CloseCode::CannotAccept => write!(f, "1003: Cannot Accept"),
            CloseCode::Reserved1 => write!(f, "1004: Reserved1"),
            CloseCode::Reserved2 => write!(f, "1005: Reserved2"),
            CloseCode::Reserved3 => write!(f, "1006: Reserved3"),
            CloseCode::InvalidUtf8 => write!(f, "1007: Invalid UTF-8"),
            CloseCode::PolicyViolation => write!(f, "1008: Policy Violation"),
            CloseCode::MessageTooBig => write!(f, "1009: Message Too Large"),
            CloseCode::ExtNegotiationFailure => write!(f, "1010: Extension Negotiation Failure"),
            CloseCode::UnexpectedFailure => write!(f, "1011: Unexpected Failure"),
            CloseCode::Reserved5 => write!(f, "1012, 1013, 1014, 1016-2999: Reserved5"),
            CloseCode::Reserved4 => write!(f, "1015: Reserved4"),
            CloseCode::AppSpecific => write!(f, "3000-3999: Application Specific"),
            CloseCode::Undefined => write!(f, "4000-4999: Application Specific"),
        }
    }
}

/// The `Close` struct.
pub struct Close<T> {
    /// A slog stdout `Logger`
    stdout: Option<Logger>,
    /// A slog stderr `Logger`
    stderr: Option<Logger>,
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
            stdout: None,
            stderr: None,
            upstream: upstream,
            received: false,
            app_data: None,
        }
    }
    /// Add a slog stdout `Logger` to this `Frame` protocol
    pub fn add_stdout(&mut self, stdout: Logger) -> &mut Close<T> {
        let c_stdout = stdout.new(o!("module" => module_path!(), "proto" => "close"));
        self.stdout = Some(c_stdout);
        self
    }

    /// Add a slog stderr `Logger` to this `Frame` protocol.
    pub fn add_stderr(&mut self, stderr: Logger) -> &mut Close<T> {
        let c_stderr = stderr.new(o!("module" => module_path!(), "proto" => "close"));
        self.stderr = Some(c_stderr);
        self
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
                            if let Some(ref stdout) = self.stdout {
                                trace!(stdout, "close message received");
                            }

                            if let Some(base) = msg.base() {
                                self.app_data = base.application_data().cloned();
                                self.received = true;
                            } else if let Some(ref stderr) = self.stderr {
                                // This should never happen.
                                error!(stderr, "couldn't extract base frame");
                            }

                            try!(self.poll_complete());
                        }
                        m => return Ok(Async::Ready(m)),
                    }
                }
                Ok(Async::NotReady) => return Ok(Async::NotReady),
                Err(e) => {
                    match e.kind() {
                        ErrorKind::Other => {
                            if let Some(ref stderr) = self.stderr {
                                error!(stderr, "{}", e.description());
                            }
                            // let mut data = Vec::with_capacity(2);
                            // if let Err(_) = data.write_u16::<BigEndian>(
                            // CloseCode::ProtocolError.into()) {
                            //     return Err(util::other("unable to write close code"));
                            // }
                            // self.app_data = Some(data);
                            // self.received = true;
                            // try!(self.poll_complete());
                        }
                        _ => {}
                    }
                    return Err(e);
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
                    let mut cc = if let Ok(len) = rdr.read_u16::<BigEndian>() {
                        CloseCode::from(len)
                    } else {
                        CloseCode::ProtocolError
                    };

                    if let Err(_) = String::from_utf8(app_data[2..].to_vec()) {
                        cc = CloseCode::ProtocolError;
                    } else {
                        rest.extend(&app_data[2..]);
                    }
                    cc
                } else {
                    CloseCode::ProtocolError
                }
            } else {
                CloseCode::Normal
            };

            let mut data = Vec::with_capacity(2);
            match close_code {
                CloseCode::Unused |
                CloseCode::ProtocolError |
                CloseCode::Reserved1 |
                CloseCode::Reserved2 |
                CloseCode::Reserved3 |
                CloseCode::Reserved4 |
                CloseCode::Reserved5 => {
                    if let Err(_) = data.write_u16::<BigEndian>(CloseCode::ProtocolError.into()) {
                        return Err(util::other("unable to write close code"));
                    }
                    data.extend(format!("{}", CloseCode::ProtocolError).bytes())
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
                                if let Some(ref stdout) = self.stdout {
                                    trace!(stdout, "received close, sending close, terminating");
                                }
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

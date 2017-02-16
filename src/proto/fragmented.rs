//! The `Fragmented` protocol middleware.
use frame::WebSocket;
use frame::base::{Frame, OpCode};
use futures::{Async, Poll, Sink, StartSend, Stream};
use slog::Logger;
use std::io;
use util;

/// The `Fragmented` struct.
pub struct Fragmented<T> {
    /// A slog stdout `Logger`
    stdout: Option<Logger>,
    /// A slog stderr `Logger`
    stderr: Option<Logger>,
    /// The upstream protocol.
    upstream: T,
    /// Has the fragmented message started?
    started: bool,
    /// Is the fragmented message complete?
    complete: bool,
    /// The `OpCode` from the original message.
    opcode: OpCode,
    /// A running total of the payload lengths.
    total_length: u64,
    /// The buffer used to store the fragmented data.
    buf: Vec<u8>,
}

impl<T> Fragmented<T> {
    /// Create a new `Fragmented` protocol middleware.
    pub fn new(upstream: T) -> Fragmented<T> {
        Fragmented {
            stdout: None,
            stderr: None,
            upstream: upstream,
            started: false,
            complete: false,
            opcode: OpCode::Close,
            total_length: 0,
            buf: Vec::new(),
        }
    }

    /// Add a slog stdout `Logger` to this `Fragmented` protocol
    pub fn add_stdout(&mut self, stdout: Logger) -> &mut Fragmented<T> {
        let fp_stdout = stdout.new(o!("module" => module_path!(), "proto" => "fragment"));
        self.stdout = Some(fp_stdout);
        self
    }

    /// Add a slog stderr `Logger` to this `Fragmented` protocol.
    pub fn add_stderr(&mut self, stderr: Logger) -> &mut Fragmented<T> {
        let fp_stderr = stderr.new(o!("module" => module_path!(), "proto" => "fragment"));
        self.stderr = Some(fp_stderr);
        self
    }
}

impl<T> Stream for Fragmented<T>
    where T: Stream<Item = WebSocket, Error = io::Error>,
          T: Sink<SinkItem = WebSocket, SinkError = io::Error>
{
    type Item = WebSocket;
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Option<WebSocket>, io::Error> {
        loop {
            match try_ready!(self.upstream.poll()) {
                Some(ref msg) if msg.is_fragment_start() => {
                    if let Some(base) = msg.base() {
                        if let Some(ref stdout) = self.stdout {
                            trace!(stdout, "fragment start frame received");
                        }
                        self.opcode = base.opcode();
                        self.started = true;
                        self.total_length += base.payload_length();
                        if let Some(app_data) = base.application_data() {
                            self.buf.extend(app_data);
                        }
                        try!(self.poll_complete());
                    } else {
                        if let Some(ref stderr) = self.stderr {
                            error!(stderr, "invalid fragment start frame received");
                        }
                        return Err(util::other("invalid fragment start frame received"));
                    }
                }
                Some(ref msg) if msg.is_fragment() => {
                    if !self.started || self.complete {
                        if let Some(ref stderr) = self.stderr {
                            error!(stderr, "invalid fragment frame received");
                        }
                        return Err(util::other("invalid fragment frame received"));
                    }

                    if let Some(base) = msg.base() {
                        if let Some(ref stdout) = self.stdout {
                            trace!(stdout, "fragment frame received");
                        }
                        self.total_length += base.payload_length();
                        if let Some(app_data) = base.application_data() {
                            self.buf.extend(app_data);
                        }
                        try!(self.poll_complete());
                    } else {
                        if let Some(ref stderr) = self.stderr {
                            error!(stderr, "invalid fragment frame received");
                        }
                        return Err(util::other("invalid fragment frame received"));
                    }
                }
                Some(ref msg) if msg.is_fragment_complete() => {
                    if !self.started || self.complete {
                        if let Some(ref stderr) = self.stderr {
                            error!(stderr, "invalid fragment complete frame received");
                        }
                        return Err(util::other("invalid fragment complete frame received"));
                    }
                    if let Some(base) = msg.base() {
                        if let Some(ref stdout) = self.stdout {
                            trace!(stdout, "fragment complete frame received");
                        }
                        self.complete = true;
                        self.total_length += base.payload_length();
                        if let Some(app_data) = base.application_data() {
                            self.buf.extend(app_data);
                        }
                        try!(self.poll_complete());
                    } else {
                        if let Some(ref stderr) = self.stderr {
                            error!(stderr, "invalid fragment complete frame received");
                        }
                        return Err(util::other("invalid fragment complete frame received"));
                    }
                }
                Some(ref msg) if msg.is_badfragment() => {
                    if self.started && !self.complete {
                        return Err(util::other("invalid opcode for continuation fragment"));
                    }
                    return Ok(Async::Ready(Some(msg.clone())));
                }
                m => return Ok(Async::Ready(m)),
            }
        }
    }
}

impl<T> Sink for Fragmented<T>
    where T: Sink<SinkItem = WebSocket, SinkError = io::Error>
{
    type SinkItem = WebSocket;
    type SinkError = io::Error;

    fn start_send(&mut self, item: WebSocket) -> StartSend<WebSocket, io::Error> {
        self.upstream.start_send(item)
    }

    fn poll_complete(&mut self) -> Poll<(), io::Error> {
        if self.started && self.complete {
            let mut coalesced: WebSocket = Default::default();
            let mut base: Frame = Default::default();

            if self.opcode == OpCode::Text {
                try!(String::from_utf8(self.buf.clone())
                    .map_err(|_| util::other("invalid utf8 in text frame")));
            }
            base.set_fin(true).set_opcode(self.opcode);
            base.set_application_data(Some(self.buf.clone()));
            base.set_payload_length(self.total_length);
            coalesced.set_base(base);
            let _ = try!(self.upstream.start_send(coalesced));
            self.started = false;
            self.complete = false;
            self.opcode = OpCode::Close;
            self.buf.clear();
            if let Some(ref stdout) = self.stdout {
                trace!(stdout, "fragment complete sending coalesced");
            }
            return self.upstream.poll_complete();
        }
        self.upstream.poll_complete()
    }
}

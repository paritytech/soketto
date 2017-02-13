//! The `Close` protocol middleware.
use frame::WebSocket;
use futures::{Async, AsyncSink, Poll, Sink, StartSend, Stream};
use slog::Logger;
use std::io;
use util;

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
            match try_ready!(self.upstream.poll()) {
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
            let mut close = WebSocket::close(self.app_data.clone());

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

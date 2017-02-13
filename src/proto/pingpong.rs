//! The `PingPong` protocol middleware.
use frame::WebSocket;
use futures::{Async, AsyncSink, Poll, Sink, StartSend, Stream};
use slog::Logger;
use std::collections::VecDeque;
use std::io;

/// The `PingPong` struct.
pub struct PingPong<T> {
    /// A slog stdout `Logger`
    stdout: Option<Logger>,
    /// A slog stderr `Logger`
    stderr: Option<Logger>,
    /// The upstream protocol.
    upstream: T,
    /// A vector of app datas for the given pings.  A pong is sent with the same data.
    app_datas: VecDeque<Option<Vec<u8>>>,
}

impl<T> PingPong<T> {
    /// Create a new `PingPong` protocol middleware.
    pub fn new(upstream: T) -> PingPong<T> {
        PingPong {
            stdout: None,
            stderr: None,
            upstream: upstream,
            app_datas: VecDeque::new(),
        }
    }

    /// Add a slog stdout `Logger` to this `Frame` protocol
    pub fn add_stdout(&mut self, stdout: Logger) -> &mut PingPong<T> {
        let pp_stdout = stdout.new(o!("module" => module_path!(), "proto" => "pingpong"));
        self.stdout = Some(pp_stdout);
        self
    }

    /// Add a slog stderr `Logger` to this `Frame` protocol.
    pub fn add_stderr(&mut self, stderr: Logger) -> &mut PingPong<T> {
        let pp_stderr = stderr.new(o!("module" => module_path!(), "proto" => "pingpong"));
        self.stderr = Some(pp_stderr);
        self
    }
}

impl<T> Stream for PingPong<T>
    where T: Stream<Item = WebSocket, Error = io::Error>,
          T: Sink<SinkItem = WebSocket, SinkError = io::Error>
{
    type Item = WebSocket;
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Option<WebSocket>, io::Error> {
        loop {
            match try_ready!(self.upstream.poll()) {
                Some(ref msg) if msg.is_ping() => {
                    if let Some(ref stdout) = self.stdout {
                        trace!(stdout, "ping message received");
                    }

                    if let Some(base) = msg.base() {
                        self.app_datas.push_back(base.application_data().cloned());
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

impl<T> Sink for PingPong<T>
    where T: Sink<SinkItem = WebSocket, SinkError = io::Error>
{
    type SinkItem = WebSocket;
    type SinkError = io::Error;

    fn start_send(&mut self, item: WebSocket) -> StartSend<WebSocket, io::Error> {
        if !self.app_datas.is_empty() {
            if let Some(ref stdout) = self.stdout {
                trace!(stdout, "sink has pending pings");
            }
            return Ok(AsyncSink::NotReady(item));
        }

        self.upstream.start_send(item)
    }

    fn poll_complete(&mut self) -> Poll<(), io::Error> {
        let mut cur = self.app_datas.pop_front();
        while let Some(app_data) = cur {
            let pong = WebSocket::pong(app_data);
            let res = try!(self.upstream.start_send(pong));

            if res.is_ready() {
                if let Some(ref stdout) = self.stdout {
                    trace!(stdout, "pong message sent");
                }
            } else {
                break;
            }
            cur = self.app_datas.pop_front();
        }

        self.upstream.poll_complete()
    }
}

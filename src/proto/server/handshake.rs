//! The `Handshake` protocol middleware.
use frame::WebSocket;
use futures::{Async, AsyncSink, Poll, Sink, StartSend, Stream};
use slog::Logger;
use std::io;

/// The `Handshake` struct.
pub struct Handshake<T> {
    /// The upstream protocol.
    upstream: T,
    /// Has the client handshake been received?
    client_received: bool,
    /// Has the server handshake response been sent?
    server_sent: bool,
    /// slog stdout `Logger`
    stdout: Option<Logger>,
    /// slog stderr `Logger`
    stderr: Option<Logger>,
}

impl<T> Handshake<T> {
    /// Create a new `Handshake` protocol middleware.
    pub fn new(upstream: T) -> Handshake<T> {
        Handshake {
            upstream: upstream,
            client_received: false,
            server_sent: false,
            stdout: None,
            stderr: None,
        }
    }

    /// Add a stdout slog `Logger` to this protocol.
    pub fn stdout(&mut self, logger: Logger) -> &mut Handshake<T> {
        let stdout = logger.new(o!("proto" => "handshake"));
        self.stdout = Some(stdout);
        self
    }

    /// Add a stderr slog `Logger` to this protocol.
    pub fn stderr(&mut self, logger: Logger) -> &mut Handshake<T> {
        let stderr = logger.new(o!("proto" => "handshake"));
        self.stderr = Some(stderr);
        self
    }
}

impl<T> Stream for Handshake<T>
    where T: Stream<Item = WebSocket, Error = io::Error>,
          T: Sink<SinkItem = WebSocket, SinkError = io::Error>
{
    type Item = WebSocket;
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Option<WebSocket>, io::Error> {
        loop {
            match try_ready!(self.upstream.poll()) {
                Some(ref msg) if msg.is_serverside_handshake_request() && !self.client_received => {
                    try_trace!(self.stdout, "client handshake request received");
                    self.client_received = true;

                    // Send the request down to the service.
                    return Ok(Async::Ready(Some(msg.clone())));
                }
                m => return Ok(Async::Ready(m)),
            }
        }
    }
}

impl<T> Sink for Handshake<T>
    where T: Sink<SinkItem = WebSocket, SinkError = io::Error>
{
    type SinkItem = WebSocket;
    type SinkError = io::Error;

    fn start_send(&mut self, item: WebSocket) -> StartSend<WebSocket, io::Error> {
        if self.server_sent {
            self.upstream.start_send(item)
        } else if self.client_received && item.is_serverside_handshake_response() {
            self.server_sent = true;
            self.upstream.start_send(item)
        } else {
            try_warn!(self.stdout, "handshake not completed");
            Ok(AsyncSink::NotReady(item))
        }
    }

    fn poll_complete(&mut self) -> Poll<(), io::Error> {
        self.upstream.poll_complete()
    }
}

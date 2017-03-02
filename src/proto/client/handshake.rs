//! client to server handshake protocol.
use frame::WebSocket;
use futures::{Async, AsyncSink, Poll, Sink, StartSend, Stream};
use slog::Logger;
use std::io;
use util;

/// The `Handshake` struct.
pub struct Handshake<T> {
    /// The upstream protocol.
    upstream: T,
    /// Has the client handshake been sent?
    client_sent: bool,
    /// Has the server handshake response been received?
    server_received: bool,
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
            client_sent: false,
            server_received: false,
            stdout: None,
            stderr: None,
        }
    }

    /// Add a stdout slog `Logger` to this protocol.
    pub fn stdout(&mut self, logger: Logger) -> &mut Handshake<T> {
        let stdout = logger.new(o!("proto" => "client::handshake"));
        self.stdout = Some(stdout);
        self
    }

    /// Add a stderr slog `Logger` to this protocol.
    pub fn stderr(&mut self, logger: Logger) -> &mut Handshake<T> {
        let stderr = logger.new(o!("proto" => "client::handshake"));
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
        try_trace!(self.stdout, "client handshake poll");
        loop {
            match try_ready!(self.upstream.poll()) {
                Some(ref msg) if msg.is_client_handshake() && !self.server_received => {
                    try_trace!(self.stdout, "server handshake message received");

                    if let Some(_handshake) = msg.client_handshake() {
                        self.server_received = true;
                        return Ok(Async::Ready(Some(msg.clone())));
                    } else {
                        return Err(util::other("couldn't extract handshake frame"));
                    }
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
        try_trace!(self.stdout, "client::handshake start_send");
        if !self.client_sent {
            self.client_sent = true;
            self.upstream.start_send(item)
        } else if self.server_received {
            self.upstream.start_send(item)
        } else {
            try_warn!(self.stdout,
                      "sink has not received server handshake response");
            Ok(AsyncSink::NotReady(item))
        }
    }

    fn poll_complete(&mut self) -> Poll<(), io::Error> {
        try_trace!(self.stdout, "client::handshake poll complete");
        self.upstream.poll_complete()
    }
}

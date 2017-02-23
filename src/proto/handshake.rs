//! The `Handshake` protocol middleware.
use frame::WebSocket;
use frame::handshake::Frame;
use futures::{Async, AsyncSink, Poll, Sink, StartSend, Stream};
use slog::Logger;
use std::io;
use util;

/// The `Handshake` struct.
pub struct Handshake<T> {
    /// The upstream protocol.
    upstream: T,
    /// Has the client handshake been received?
    client_received: bool,
    /// Has the server handshake response been sent?
    server_sent: bool,
    /// The client handshake frame.  Used to generate the proper response.
    client_handshake: Frame,
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
            client_handshake: Default::default(),
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
                Some(ref msg) if msg.is_handshake() && !self.client_received => {
                    try_trace!(self.stdout, "client handshake message received");

                    if let Some(handshake) = msg.handshake() {
                        self.client_received = true;
                        self.client_handshake = handshake.clone();
                    } else {
                        return Err(util::other("couldn't extract handshake frame"));
                    }

                    try!(self.poll_complete());
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
        try_trace!(self.stdout, "start_send");
        if !self.server_sent || !self.client_received {
            try_warn!(self.stdout,
                      "sink has not received client handshake request");
            return Ok(AsyncSink::NotReady(item));
        }

        self.upstream.start_send(item)
    }

    fn poll_complete(&mut self) -> Poll<(), io::Error> {
        if self.client_received && !self.server_sent {
            let mut handshake_resp = WebSocket::handshake_resp(self.client_handshake.clone());
            loop {
                let res = try!(self.upstream.start_send(handshake_resp));
                match res {
                    AsyncSink::Ready => {
                        loop {
                            if let Ok(Async::Ready(_)) = self.upstream.poll_complete() {
                                try_trace!(self.stdout,
                                           "received client handshake request, \
                                    sending server handshake response");
                                self.server_sent = true;
                                return Ok(Async::Ready(()));
                            }
                        }
                    }
                    AsyncSink::NotReady(v) => handshake_resp = v,
                }
            }
        }
        self.upstream.poll_complete()
    }
}

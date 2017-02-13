//! The `Handshake` protocol middleware.
use frame::WebSocket;
use frame::handshake::Frame;
use futures::{Async, AsyncSink, Poll, Sink, StartSend, Stream};
use slog::Logger;
use std::io;

/// The `Handshake` struct.
pub struct Handshake<T> {
    /// A slog stdout `Logger`
    stdout: Option<Logger>,
    /// A slog stdout `Logger`
    stderr: Option<Logger>,
    /// The upstream protocol.
    upstream: T,
    /// Has the client handshake been received?
    client_received: bool,
    /// Has the server handshake response been sent?
    server_sent: bool,
    /// The client handshake frame.  Used to generate the proper response.
    client_handshake: Frame,
}

impl<T> Handshake<T> {
    /// Create a new `Handshake` protocol middleware.
    pub fn new(upstream: T) -> Handshake<T> {
        Handshake {
            stdout: None,
            stderr: None,
            upstream: upstream,
            client_received: false,
            server_sent: false,
            client_handshake: Default::default(),
        }
    }
    /// Add a slog stdout `Logger` to this `Handshake` protocol
    pub fn add_stdout(&mut self, stdout: Logger) -> &mut Handshake<T> {
        let h_stdout = stdout.new(o!("module" => module_path!(), "proto" => "handshake"));
        self.stdout = Some(h_stdout);
        self
    }

    /// Add a slog stderr `Logger` to this `Handshake` protocol.
    pub fn add_stderr(&mut self, stderr: Logger) -> &mut Handshake<T> {
        let h_stderr = stderr.new(o!("module" => module_path!(), "proto" => "handshake"));
        self.stderr = Some(h_stderr);
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
                    if let Some(ref stdout) = self.stdout {
                        trace!(stdout, "client handshake message received\n{}", msg);
                    }

                    if let Some(handshake) = msg.handshake() {
                        self.client_received = true;
                        self.client_handshake = handshake.clone();
                    } else if let Some(ref stderr) = self.stderr {
                        // This should never happen.
                        error!(stderr, "couldn't extract handshake frame");
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
        if !self.client_received {
            if let Some(ref stdout) = self.stdout {
                trace!(stdout, "sink has not received client handshake request");
            }
            return Ok(AsyncSink::NotReady(item));
        } else if self.client_received && !self.server_sent {
            if let Some(ref stdout) = self.stdout {
                trace!(stdout, "sink has not sent server handshake response");
            }
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
                                if let Some(ref stdout) = self.stdout {
                                    trace!(stdout,
                                           "received client handshake request, \
                                sent server handshake response");
                                }
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

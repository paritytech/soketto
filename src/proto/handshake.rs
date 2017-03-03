//! The `Handshake` protocol middleware.
use frame::WebSocket;
// use frame::server::response::Frame as ServerSideHandshakeResponse;
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
    /// `Sec-WebSocket-Key` from request.
    ws_key: String,
    /// Extensions from request.
    extensions: String,
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
            ws_key: String::new(),
            extensions: String::new(),
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
                Some(ref msg) if msg.is_server_handshake_request() && !self.client_received => {
                    try_trace!(self.stdout, "client handshake request received");

                    if let Some(handshake) = msg.serverside_handshake_request() {
                        self.client_received = true;
                        self.ws_key = handshake.ws_key().into();
                        self.extensions = handshake.extensions().into();
                    } else {
                        return Err(util::other("couldn't extract handshake frame"));
                    }

                    // Send the repsonse upstream, and send the request to the service.
                    self.poll_complete()?;
                    return Ok(Async::Ready(Some(msg.clone())));
                }
                Some(ref msg) if msg.is_server_handshake_response() && self.client_received => {
                    try_trace!(self.stdout, "server handshake response received");
                    try_trace!(self.stdout, "{}", msg);
                    self.server_sent = true;
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
        try_trace!(self.stdout, "start_send");
        if !self.server_sent || !self.client_received {
            try_warn!(self.stdout,
                      "sink has not received client handshake request");
            return Ok(AsyncSink::NotReady(item));
        }

        self.upstream.start_send(item)
    }

    fn poll_complete(&mut self) -> Poll<(), io::Error> {
        // if self.client_received && !self.server_sent {
        //     let mut frame: WebSocket = Default::default();
        //     let mut resp: ServerSideHandshakeResponse = Default::default();
        //     resp.set_ws_key(self.ws_key.clone());
        //     resp.set_extensions(self.extensions.clone());
        //     frame.set_serverside_handshake_response(resp);
        //     loop {
        //         let res = self.upstream.start_send(frame)?;
        //         match res {
        //             AsyncSink::Ready => {
        //                 loop {
        //                     if let Ok(Async::Ready(_)) = self.upstream.poll_complete() {
        //                         try_trace!(self.stdout,
        //                                    "received client handshake request,
        //                             sending server handshake response");
        //                         self.server_sent = true;
        //                         return Ok(Async::Ready(()));
        //                     }
        //                 }
        //             }
        //             AsyncSink::NotReady(v) => frame = v,
        //         }
        //     }
        // }
        self.upstream.poll_complete()
    }
}

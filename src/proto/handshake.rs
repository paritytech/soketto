use frame::WebSocketFrame;
use futures::{Async, Poll, Sink, StartSend, Stream};
use slog::Logger;
use std::io;

pub struct HandshakeProto<T> {
    stdout: Logger,
    stderr: Logger,
    upstream: T,
    client_received: bool,
    #[allow(dead_code)]
    server_sent: bool,
}

impl<T> HandshakeProto<T> {
    pub fn new(stdout: Logger, stderr: Logger, upstream: T) -> HandshakeProto<T> {
        let wsp_stdout = stdout.new(o!("proto" => "handshake"));
        let wsp_stderr = stderr.new(o!("proto" => "handshake"));
        HandshakeProto {
            stdout: wsp_stdout,
            stderr: wsp_stderr,
            upstream: upstream,
            client_received: false,
            server_sent: false,
        }
    }
}

impl<T> Stream for HandshakeProto<T>
    where T: Stream<Item = WebSocketFrame, Error = io::Error>,
          T: Sink<SinkItem = WebSocketFrame, SinkError = io::Error>
{
    type Item = WebSocketFrame;
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Option<WebSocketFrame>, io::Error> {
        trace!(self.stdout, "stream poll");
        loop {
            match try_ready!(self.upstream.poll()) {
                Some(ref msg) if msg.is_handshake() && !self.client_received => {
                    trace!(self.stdout, "client handshake message received");

                    if let Some(_handshake) = msg.handshake() {
                        self.client_received = true;
                    } else {
                        // This should never happen.
                        error!(self.stderr, "couldn't extract handshake frame");
                    }

                    try!(self.poll_complete());
                }
                m => return Ok(Async::Ready(m)),
            }
        }
    }
}

impl<T> Sink for HandshakeProto<T>
    where T: Sink<SinkItem = WebSocketFrame, SinkError = io::Error>
{
    type SinkItem = WebSocketFrame;
    type SinkError = io::Error;

    fn start_send(&mut self, item: WebSocketFrame) -> StartSend<WebSocketFrame, io::Error> {
        trace!(self.stdout, "sink start_send");
        self.upstream.start_send(item)
    }

    fn poll_complete(&mut self) -> Poll<(), io::Error> {
        trace!(self.stdout, "sink poll_complete");
        self.upstream.poll_complete()

    }
}

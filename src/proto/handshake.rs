use frame::WebSocketFrame;
use frame::handshake::HandshakeFrame;
use futures::{Async, AsyncSink, Poll, Sink, StartSend, Stream};
use slog::Logger;
use std::io;

pub struct HandshakeProto<T> {
    stdout: Logger,
    stderr: Logger,
    upstream: T,
    client_received: bool,
    server_sent: bool,
    client_handshake: HandshakeFrame,
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
            client_handshake: Default::default(),
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

                    if let Some(handshake) = msg.handshake() {
                        self.client_received = true;
                        self.client_handshake = handshake.clone();
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

        if !self.client_received {
            trace!(self.stdout,
                   "sink has not received client handshake request");
            return Ok(AsyncSink::NotReady(item));
        } else if self.client_received && !self.server_sent {
            trace!(self.stdout, "sink has not sent server handshake response");
            return Ok(AsyncSink::NotReady(item));
        }

        self.upstream.start_send(item)
    }

    fn poll_complete(&mut self) -> Poll<(), io::Error> {
        trace!(self.stdout, "sink poll_complete");
        if self.client_received && !self.server_sent {
            let mut handshake_resp = WebSocketFrame::handshake_resp(self.client_handshake.clone());
            loop {
                let res = try!(self.upstream.start_send(handshake_resp));
                match res {
                    AsyncSink::Ready => {
                        loop {
                            let res = self.upstream.poll_complete();

                            match res {
                                Ok(Async::Ready(_)) => {
                                    trace!(self.stdout,
                                           "received client handshake request, \
                                    sent server handshake response");
                                    self.server_sent = true;
                                    return Ok(Async::Ready(()));
                                }
                                _ => {
                                    // loop until ready so we can close
                                }
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

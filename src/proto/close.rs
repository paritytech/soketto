use frame::WebSocketFrame;
use futures::{Async, Poll, Sink, StartSend, Stream};
use slog::Logger;
use std::io;

fn other(desc: &str) -> io::Error {
    io::Error::new(io::ErrorKind::Other, desc)
}

pub struct CloseProto<T> {
    stdout: Logger,
    stderr: Logger,
    upstream: T,
    received: bool,
    app_data: Option<Vec<u8>>,
}

impl<T> CloseProto<T> {
    pub fn new(stdout: Logger, stderr: Logger, upstream: T) -> CloseProto<T> {
        let wsp_stdout = stdout.new(o!("proto" => "close"));
        let wsp_stderr = stderr.new(o!("proto" => "close"));
        CloseProto {
            stdout: wsp_stdout,
            stderr: wsp_stderr,
            upstream: upstream,
            received: false,
            app_data: None,
        }
    }
}

impl<T> Stream for CloseProto<T>
    where T: Stream<Item = WebSocketFrame, Error = io::Error>,
          T: Sink<SinkItem = WebSocketFrame, SinkError = io::Error>
{
    type Item = WebSocketFrame;
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Option<WebSocketFrame>, io::Error> {
        trace!(self.stdout, "stream poll");
        loop {
            match try_ready!(self.upstream.poll()) {
                Some(ref msg) if msg.is_close() => {
                    trace!(self.stdout, "close message received");

                    if let Some(base) = msg.base() {
                        self.app_data = base.application_data().cloned();
                        self.received = true;
                    } else {
                        // This should never happen.
                        error!(self.stderr, "couldn't extract base frame");
                    }

                    try!(self.poll_complete());
                }
                m => return Ok(Async::Ready(m)),
            }
        }
    }
}

impl<T> Sink for CloseProto<T>
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
        if self.received {
            let close = WebSocketFrame::close(self.app_data.clone());
            let res = try!(self.upstream.start_send(close));
        }

        self.upstream.poll_complete()
    }
}

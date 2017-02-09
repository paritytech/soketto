use frame::WebSocketFrame;
use futures::{Async, AsyncSink, Poll, Sink, StartSend, Stream};
use slog::Logger;
use std::collections::VecDeque;
use std::io;

pub struct PingPongProto<T> {
    stdout: Logger,
    stderr: Logger,
    upstream: T,
    app_datas: VecDeque<Option<Vec<u8>>>,
}

impl<T> PingPongProto<T> {
    pub fn new(stdout: Logger, stderr: Logger, upstream: T) -> PingPongProto<T> {
        let cfp_stdout = stdout.new(o!("proto" => "pingpong"));
        let cfp_stderr = stderr.new(o!("proto" => "pingpong"));
        PingPongProto {
            stdout: cfp_stdout,
            stderr: cfp_stderr,
            upstream: upstream,
            app_datas: VecDeque::new(),
        }
    }
}

impl<T> Stream for PingPongProto<T>
    where T: Stream<Item = WebSocketFrame, Error = io::Error>,
          T: Sink<SinkItem = WebSocketFrame, SinkError = io::Error>
{
    type Item = WebSocketFrame;
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Option<WebSocketFrame>, io::Error> {
        trace!(self.stdout, "stream poll");
        loop {
            match try_ready!(self.upstream.poll()) {
                Some(ref msg) if msg.is_ping() => {
                    trace!(self.stdout, "ping message received");

                    if let Some(base) = msg.base() {
                        self.app_datas.push_back(base.application_data().cloned());
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

impl<T> Sink for PingPongProto<T>
    where T: Sink<SinkItem = WebSocketFrame, SinkError = io::Error>
{
    type SinkItem = WebSocketFrame;
    type SinkError = io::Error;

    fn start_send(&mut self, item: WebSocketFrame) -> StartSend<WebSocketFrame, io::Error> {
        trace!(self.stdout, "sink start_send");

        if !self.app_datas.is_empty() {
            trace!(self.stdout, "sink has pending pings");
            return Ok(AsyncSink::NotReady(item));
        }

        self.upstream.start_send(item)
    }

    fn poll_complete(&mut self) -> Poll<(), io::Error> {
        trace!(self.stdout, "sink poll_complete");
        let mut cur = self.app_datas.pop_front();
        loop {
            match cur {
                Some(app_data) => {
                    let pong = WebSocketFrame::pong(app_data);
                    let res = try!(self.upstream.start_send(pong));

                    if !res.is_ready() {
                        break;
                    }
                }
                None => break,
            }
            cur = self.app_datas.pop_front();
        }

        self.upstream.poll_complete()
    }
}

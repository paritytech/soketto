use frame::{FrameCodec, WebsocketFrame};
use slog::Logger;
use std::io;
use tokio_core::io::{Framed, Io};
use tokio_proto::streaming::pipeline::ServerProto;

pub struct WebSocketProto {
    stdout: Logger,
}

impl WebSocketProto {
    pub fn new(stdout: Logger) -> WebSocketProto {
        WebSocketProto { stdout: stdout }
    }
}

impl<T: Io + 'static> ServerProto<T> for WebSocketProto {
    type Request = WebsocketFrame;
    type RequestBody = WebsocketFrame;
    type Response = WebsocketFrame;
    type ResponseBody = WebsocketFrame;
    type Error = io::Error;

    type Transport = Framed<T, FrameCodec>;
    type BindTransport = Result<Self::Transport, io::Error>;

    fn bind_transport(&self, io: T) -> Self::BindTransport {
        trace!(self.stdout, "bind_transport");
        // Initialize the codec to be parsing message frames
        let codec: FrameCodec = Default::default();
        Ok(io.framed(codec))
    }
}

use frame::{FrameCodec, WebsocketFrame};
use std::io;
use tokio_core::io::{Framed, Io};
use tokio_proto::streaming::pipeline::ServerProto;

pub struct WebSocketProto;

impl<T: Io + 'static> ServerProto<T> for WebSocketProto {
    type Request = WebsocketFrame;
    type RequestBody = WebsocketFrame;
    type Response = WebsocketFrame;
    type ResponseBody = WebsocketFrame;
    type Error = io::Error;

    type Transport = Framed<T, FrameCodec>;
    type BindTransport = Result<Self::Transport, io::Error>;

    fn bind_transport(&self, io: T) -> Self::BindTransport {
        // Initialize the codec to be parsing message frames
        let codec: FrameCodec = Default::default();
        Ok(io.framed(codec))
    }
}

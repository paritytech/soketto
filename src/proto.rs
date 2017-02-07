use frame::{FrameCodec, WebsocketFrame};
use slog::Logger;
use std::io;
use tokio_core::io::{Framed, Io};
use tokio_proto::streaming::pipeline::ServerProto;

pub struct WebSocketProto {
    stdout: Logger,
    stderr: Logger,
}

impl WebSocketProto {
    pub fn new(stdout: Logger, stderr: Logger) -> WebSocketProto {
        let wsp_stdout = stdout.new(o!("module" => module_path!()));
        let wsp_stderr = stderr.new(o!("module" => module_path!()));
        WebSocketProto {
            stdout: wsp_stdout,
            stderr: wsp_stderr,
        }
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
        let mut codec: FrameCodec = Default::default();
        codec.add_stdout(self.stdout.clone()).add_stderr(self.stderr.clone());
        Ok(io.framed(codec))
    }
}

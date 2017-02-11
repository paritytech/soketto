use frame::{FrameCodec, WebSocketFrame};
use proto::close::CloseProto;
use proto::handshake::HandshakeProto;
use proto::pingpong::PingPongProto;
use slog::Logger;
use std::io;
use tokio_core::io::{Framed, Io};
use tokio_proto::pipeline::ServerProto;

mod close;
mod handshake;
mod pingpong;

pub struct FrameProto {
    stdout: Logger,
    stderr: Logger,
}

impl FrameProto {
    pub fn new(stdout: Logger, stderr: Logger) -> FrameProto {
        let fp_stdout = stdout.new(o!("module" => module_path!(), "proto" => "frame"));
        let fp_stderr = stderr.new(o!("module" => module_path!(), "proto" => "frame"));
        FrameProto {
            stdout: fp_stdout,
            stderr: fp_stderr,
        }
    }
}

type ProtoChain<T> = HandshakeProto<CloseProto<PingPongProto<Framed<T, FrameCodec>>>>;

impl<T: Io + 'static> ServerProto<T> for FrameProto {
    type Request = WebSocketFrame;
    type Response = WebSocketFrame;

    type Transport = ProtoChain<T>;
    type BindTransport = Result<Self::Transport, io::Error>;

    fn bind_transport(&self, io: T) -> Self::BindTransport {
        trace!(self.stdout, "bind_transport");
        // Initialize the codec to be parsing message frames
        let mut codec: FrameCodec = Default::default();
        codec.add_stdout(self.stdout.clone()).add_stderr(self.stderr.clone());

        let base = io.framed(codec);
        let pingpong = PingPongProto::new(self.stdout.clone(), self.stderr.clone(), base);
        let close = CloseProto::new(self.stdout.clone(), self.stderr.clone(), pingpong);
        let hand = HandshakeProto::new(self.stdout.clone(), self.stderr.clone(), close);

        Ok(hand)
    }
}

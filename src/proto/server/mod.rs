//! server specific tokio-proto protocols
use codec::Twist;
use frame::WebSocket;
use proto::server::close::Close;
use proto::server::fragmented::Fragmented;
use proto::server::handshake::Handshake;
use proto::server::pingpong::PingPong;
use std::io;
use tokio_io::{AsyncRead, AsyncWrite};
use tokio_io::codec::Framed;
use tokio_proto::pipeline::ServerProto;

pub use super::WebSocketProtocol;

mod close;
mod handshake;
mod fragmented;
mod pingpong;

/// The base codec type.
type BaseCodec<T> = Framed<T, Twist>;
/// The websocket protocol middleware chain type.
type ProtoChain<T> = Handshake<Close<PingPong<Fragmented<BaseCodec<T>>>>>;

impl<T: AsyncRead + AsyncWrite + 'static> ServerProto<T> for WebSocketProtocol {
    type Request = WebSocket;
    type Response = WebSocket;

    type Transport = ProtoChain<T>;
    type BindTransport = Result<Self::Transport, io::Error>;

    fn bind_transport(&self, io: T) -> Self::BindTransport {
        try_trace!(self.stdout, "server bind_transport");

        // Setup the twist codec.
        let mut twist: Twist = Twist::new(self.uuid,
                                          self.client,
                                          self.permessage_extensions.clone(),
                                          self.perframe_extensions.clone());
        if let Some(ref stdout) = self.stdout {
            twist.stdout(stdout.clone());
        }
        if let Some(ref stderr) = self.stderr {
            twist.stderr(stderr.clone());
        }

        // Setup the fragmented middleware.
        let mut fragmented = Fragmented::new(io.framed(twist),
                                             self.uuid,
                                             self.permessage_extensions.clone(),
                                             self.perframe_extensions.clone());
        if let Some(ref stdout) = self.stdout {
            fragmented.stdout(stdout.clone());
        }
        if let Some(ref stderr) = self.stderr {
            fragmented.stderr(stderr.clone());
        }

        // Setup the pingpong middleware.
        let mut pingpong = PingPong::new(fragmented);
        if let Some(ref stdout) = self.stdout {
            pingpong.stdout(stdout.clone());
        }
        if let Some(ref stderr) = self.stderr {
            pingpong.stderr(stderr.clone());
        }

        // Setup the close middleware.
        let mut close = Close::new(pingpong);
        if let Some(ref stdout) = self.stdout {
            close.stdout(stdout.clone());
        }
        if let Some(ref stderr) = self.stderr {
            close.stderr(stderr.clone());
        }

        // Setup the handshake middleware.
        let mut handshake = Handshake::new(close);
        if let Some(ref stdout) = self.stdout {
            handshake.stdout(stdout.clone());
        }
        if let Some(ref stderr) = self.stderr {
            handshake.stderr(stderr.clone());
        }

        /// Setup the protocol middleware chain.
        Ok(handshake)
    }
}

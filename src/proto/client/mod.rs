//! client specific tokio-proto protocols
use codec::Twist;
use frame::WebSocket;
use proto::client::handshake::Handshake as ClientHandshake;
use std::io;
use tokio_core::io::{Framed, Io};
use tokio_proto::pipeline::ClientProto;

pub use super::WebSocketProtocol;
pub mod handshake;

impl<T: Io + 'static> ClientProto<T> for WebSocketProtocol {
    type Request = WebSocket;
    type Response = WebSocket;

    type Transport = ClientHandshake<Framed<T, Twist>>;
    type BindTransport = Result<Self::Transport, io::Error>;

    fn bind_transport(&self, io: T) -> Self::BindTransport {
        try_trace!(self.stdout, "client bind_transport");
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

        // Setup the client handshake middleware.
        let mut handshake = ClientHandshake::new(io.framed(twist));
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

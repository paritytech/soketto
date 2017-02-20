//! A `ServerProto` implementation for the websocket protocol.
//!
//! The protocol is actually a chain of middleware protocols:
//!
//! 1. `Handshake`:  Handles the client handshake request, and the server handshake
//!    response.  Any inital request must be well-formed or the connection is terminated.  See
//!    [Section 4.2.1][open] in RFC6455 for the opening hanshake request requirements.  If the
//!    request is well-formed, the server will generate an HTTP response conforming to the
//!    requirements in [Section 4.2.2][resp] of RFC6455.
//! 2. `Close`: Handles a client close frame, and generates the server close response.
//! 3. `PingPong`: Handles a client ping frame, and generates the server pong frame
//!    response.
//! 4. `Fragmented`: Handles fragmented client frames, and generates the server coalesced
//!    response.
//! 5. `Frame`: Everything else (text and binary frames), are handled by this protocol.
//!
//! [open]: https://tools.ietf.org/html/rfc6455#section-4.2.1
//! [resp]: https://tools.ietf.org/html/rfc6455#section-4.2.2
use codec::Twist;
use frame::WebSocket;
use proto::close::Close;
use proto::fragmented::Fragmented;
use proto::handshake::Handshake;
use proto::pingpong::PingPong;
use std::io;
use tokio_core::io::{Framed, Io};
use tokio_proto::pipeline::ServerProto;

pub use util::set_stdout_level;

mod close;
mod handshake;
mod fragmented;
mod pingpong;

/// The protocol that you should run a tokio-proto
/// [`TcpServer`](https://docs.rs/tokio-proto/0.1.0/tokio_proto/struct.TcpServer.html) with to
/// handle websocket handshake and base frames.
#[derive(Default)]
pub struct WebSocketProtocol;

/// The base codec type.
type BaseCodec<T> = Framed<T, Twist>;
/// The websocket protocol middleware chain type.
type ProtoChain<T> = Handshake<Close<PingPong<Fragmented<BaseCodec<T>>>>>;

impl<T: Io + 'static> ServerProto<T> for WebSocketProtocol {
    type Request = WebSocket;
    type Response = WebSocket;

    type Transport = ProtoChain<T>;
    type BindTransport = Result<Self::Transport, io::Error>;

    fn bind_transport(&self, io: T) -> Self::BindTransport {
        stdout_trace!("proto" => "server"; "bind_transport");
        Ok(Handshake::new(
            Close::new(PingPong::new(Fragmented::new(io.framed(Default::default()))))))
    }
}

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
use ext::{PerFrame, PerFrameExtensions, PerMessage, PerMessageExtensions};
use frame::WebSocket;
use proto::close::Close;
use proto::fragmented::Fragmented;
use proto::handshake::Handshake;
use proto::pingpong::PingPong;
use slog::Logger;
use std::collections::HashMap;
use std::io;
use std::sync::{Arc, Mutex};
use tokio_core::io::{Framed, Io};
use tokio_proto::pipeline::ServerProto;
use uuid::Uuid;

mod close;
mod handshake;
mod fragmented;
mod pingpong;

/// The protocol that you should run a tokio-proto
/// [`TcpServer`](https://docs.rs/tokio-proto/0.1.0/tokio_proto/struct.TcpServer.html) with to
/// handle websocket handshake and base frames.
pub struct WebSocketProtocol {
    /// The UUID of this `WebSocketProtocol`
    uuid: Uuid,
    /// Per-message extensions
    permessage_extensions: PerMessageExtensions,
    /// Per-frame extensions
    perframe_extensions: PerFrameExtensions,
    /// slog stdout `Logger`
    stdout: Option<Logger>,
    /// slog stderr `Logger`
    stderr: Option<Logger>,
}

impl WebSocketProtocol {
    /// Add a stdout slog `Logger` to this protocol.
    pub fn stdout(&mut self, logger: Logger) -> &mut WebSocketProtocol {
        let stdout = logger.new(o!("proto" => "websocketprotocol"));
        self.stdout = Some(stdout);
        self
    }

    /// Add a stderr slog `Logger` to this protocol.
    pub fn stderr(&mut self, logger: Logger) -> &mut WebSocketProtocol {
        let stderr = logger.new(o!("proto" => "websocketprotocol"));
        self.stderr = Some(stderr);
        self
    }

    /// Register a per-message extension.
    pub fn per_message<T>(&mut self, extension: T) -> &mut WebSocketProtocol
        where T: PerMessage + 'static
    {
        let pm_lock = self.permessage_extensions.clone();
        let mut map = match pm_lock.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        let mut vec = map.entry(self.uuid).or_insert_with(Vec::new);
        vec.push(Box::new(extension));
        self
    }

    /// Register a per-frame extension.
    pub fn per_frame<T>(&mut self, extension: T) -> &mut WebSocketProtocol
        where T: PerFrame + 'static
    {
        let pf_lock = self.perframe_extensions.clone();
        let mut map = match pf_lock.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        let mut vec = map.entry(self.uuid).or_insert_with(Vec::new);
        vec.push(Box::new(extension));
        self
    }
}

impl Default for WebSocketProtocol {
    fn default() -> WebSocketProtocol {
        WebSocketProtocol {
            uuid: Uuid::new_v4(),
            permessage_extensions: Arc::new(Mutex::new(HashMap::new())),
            perframe_extensions: Arc::new(Mutex::new(HashMap::new())),
            stdout: None,
            stderr: None,
        }
    }
}

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
        try_trace!(self.stdout, "bind_transport");

        // Setup the twist codec.
        let mut twist: Twist = Twist::new(self.uuid,
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

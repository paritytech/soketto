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
use slog::Logger;
use std::io;
use tokio_core::io::{Framed, Io};
use tokio_proto::pipeline::ServerProto;

mod close;
mod fragmented;
mod handshake;
mod pingpong;

/// The `Frame` protocol that you should run a `TcpServer` with.
pub struct Frame {
    /// An optional slog stdout `Logger`
    stdout: Option<Logger>,
    /// An optional slog stderr `Logger`
    stderr: Option<Logger>,
}

impl Frame {
    /// Add a slog stdout `Logger` to this `Frame` protocol
    pub fn add_stdout(&mut self, stdout: Logger) -> &mut Frame {
        let fp_stdout = stdout.new(o!("module" => module_path!(), "proto" => "frame"));
        self.stdout = Some(fp_stdout);
        self
    }

    /// Add a slog stderr `Logger` to this `Frame` protocol.
    pub fn add_stderr(&mut self, stderr: Logger) -> &mut Frame {
        let fp_stderr = stderr.new(o!("module" => module_path!(), "proto" => "frame"));
        self.stderr = Some(fp_stderr);
        self
    }
}

impl Default for Frame {
    fn default() -> Frame {
        Frame {
            stdout: None,
            stderr: None,
        }
    }
}

/// The base codec type.
type BaseCodec<T> = Framed<T, Twist>;
/// The websocket protocol middleware chain type.
type ProtoChain<T> = Handshake<Close<PingPong<Fragmented<BaseCodec<T>>>>>;

impl<T: Io + 'static> ServerProto<T> for Frame {
    type Request = WebSocket;
    type Response = WebSocket;

    type Transport = ProtoChain<T>;
    type BindTransport = Result<Self::Transport, io::Error>;

    fn bind_transport(&self, io: T) -> Self::BindTransport {
        if let Some(ref stdout) = self.stdout {
            trace!(stdout, "bind_transport");
        }
        // Initialize the codec to be parsing message frames
        let mut codec: Twist = Default::default();
        if let Some(ref stdout) = self.stdout {
            codec.add_stdout(stdout.clone());
        }
        if let Some(ref stderr) = self.stderr {
            codec.add_stderr(stderr.clone());
        }

        let mut fragmented = Fragmented::new(io.framed(codec));
        if let Some(ref stdout) = self.stdout {
            fragmented.add_stdout(stdout.clone());
        }
        if let Some(ref stderr) = self.stderr {
            fragmented.add_stderr(stderr.clone());
        }

        let mut pingpong = PingPong::new(fragmented);
        if let Some(ref stdout) = self.stdout {
            pingpong.add_stdout(stdout.clone());
        }
        if let Some(ref stderr) = self.stderr {
            pingpong.add_stderr(stderr.clone());
        }

        let mut close = Close::new(pingpong);
        if let Some(ref stdout) = self.stdout {
            close.add_stdout(stdout.clone());
        }
        if let Some(ref stderr) = self.stderr {
            close.add_stderr(stderr.clone());
        }

        let mut hand = Handshake::new(close);
        if let Some(ref stdout) = self.stdout {
            hand.add_stdout(stdout.clone());
        }
        if let Some(ref stderr) = self.stderr {
            hand.add_stderr(stderr.clone());
        }

        Ok(hand)
    }
}

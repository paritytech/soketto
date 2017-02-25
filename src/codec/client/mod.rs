//! Codec for use with the `WebSocketProtocol`.  Used when decoding/encoding of both websocket
//! handshakes and websocket base frames on the client side.
use codec::base::FrameCodec;
use ext::{PerFrameExtensions, PerMessageExtensions};
use slog::Logger;
use uuid::Uuid;

pub mod handshake;

/// Codec for use with the [`WebSocketProtocol`](struct.WebSocketProtocol.html).  Used when
/// decoding/encoding of both websocket handshakes and websocket base frames on the client side.
#[derive(Default)]
pub struct Twist {
    /// The Uuid of the parent protocol.  Used for extension lookup.
    uuid: Uuid,
    /// The handshake indicator.  If this is false, the handshake is not complete.
    shaken: bool,
    /// The `Codec` to use to decode the buffer into a `base::Frame`.  Due to the reentrant nature
    /// of decode, the codec is initialized if set to None, reused if Some, and reset to None
    /// after every successful decode.
    frame_codec: Option<FrameCodec>,
    /// The `Codec` use to decode the buffer into a `handshake::Frame`.  Due to the reentrant nature
    /// of decode, the codec is initialized if set to None, reused if Some, and reset to None
    /// after every successful decode.
    handshake_codec: Option<handshake::FrameCodec>,
    /// The `Origin` header, if present.
    origin: Option<String>,
    /// Per-message extensions
    permessage_extensions: PerMessageExtensions,
    /// Per-frame extensions
    perframe_extensions: PerFrameExtensions,
    /// RSVx bits reserved by extensions (must be less than 16)
    reserved_bits: u8,
    /// slog stdout `Logger`
    stdout: Option<Logger>,
    /// slog stderr `Logger`
    stderr: Option<Logger>,
}

impl Twist {
    /// Create a new `Twist` codec with the given extensions.
    pub fn new(uuid: Uuid,
               permessage_extensions: PerMessageExtensions,
               perframe_extensions: PerFrameExtensions)
               -> Twist {
        Twist {
            uuid: uuid,
            permessage_extensions: permessage_extensions,
            perframe_extensions: perframe_extensions,
            ..Default::default()
        }
    }

    /// Add a stdout slog `Logger` to this protocol.
    pub fn stdout(&mut self, logger: Logger) -> &mut Twist {
        let stdout = logger.new(o!("codec" => "Twist"));
        self.stdout = Some(stdout);
        self
    }

    /// Add a stderr slog `Logger` to this protocol.
    pub fn stderr(&mut self, logger: Logger) -> &mut Twist {
        let stderr = logger.new(o!("codec" => "Twist"));
        self.stderr = Some(stderr);
        self
    }
}

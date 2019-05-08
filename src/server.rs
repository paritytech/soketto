//! Server API
//!
//! # Decode PING Base Frame from client
//!
//! ```
//! # use bytes::BytesMut;
//! # use twist::server::{BaseFrameCodec, OpCode};
//! # use tokio_io::codec::{Decoder, Encoder};
//! #
//! # const PING: [u8; 6] = [0x89, 0x80, 0x00, 0x00, 0x00, 0x01];
//! #
//! # fn main() {
//!     let ping_vec = PING.to_vec();
//!     let mut eb = BytesMut::with_capacity(256);
//!     eb.extend(ping_vec);
//!     let mut fc: BaseFrameCodec = Default::default();
//!     let mut encoded = BytesMut::with_capacity(256);
//!
//!     if let Ok(Some(frame)) = fc.decode(&mut eb) {
//!         assert!(frame.fin());
//!         assert!(!frame.rsv1());
//!         assert!(!frame.rsv2());
//!         assert!(!frame.rsv3());
//!         // All frames from client must be masked.
//!         assert!(frame.masked());
//!         assert!(frame.opcode() == OpCode::Ping);
//!         assert!(frame.mask() == 1);
//!         assert!(frame.payload_length() == 0);
//!         assert!(frame.extension_data().is_none());
//!         assert!(frame.application_data().is_empty());
//!
//!         if fc.encode(frame, &mut encoded).is_ok() {
//!             for (a, b) in encoded.iter().zip(PING.to_vec().iter()) {
//!                 assert!(a == b);
//!             }
//!         }
//!     } else {
//!         assert!(false);
//!     }
//! # }
//! ```
// Common Codec Exports
pub use crate::codec::Twist as TwistCodec;
pub use crate::codec::base::FrameCodec as BaseFrameCodec;

// Server Only Codec Exports
pub use crate::codec::server::handshake::FrameCodec as HandshakeCodec;

// Common Frame Exports
pub use crate::frame::WebSocket as WebSocketFrame;
pub use crate::frame::base::Frame as BaseFrame;
pub use crate::frame::base::OpCode;

// Server Only Frame Exports
pub use crate::frame::server::request::Frame as HandshakeRequestFrame;
pub use crate::frame::server::response::Frame as HandshakeResponseFrame;


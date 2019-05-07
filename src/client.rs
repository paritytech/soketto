//! Client API
//!
//! # Decode PONG Base Frame from server
//!
//! ```
//! # extern crate bytes;
//! # extern crate tokio_io;
//! # extern crate twist;
//! #
//! # use bytes::BytesMut;
//! # use twist::client::{BaseFrameCodec, OpCode};
//! # use tokio_io::codec::{Decoder, Encoder};
//! #
//! # const PONG: [u8; 2] = [0x8a, 0x00];
//! #
//! # fn main() {
//!     let ping_vec = PONG.to_vec();
//!     let mut eb = BytesMut::with_capacity(256);
//!     eb.extend(ping_vec);
//!     let mut fc: BaseFrameCodec = Default::default();
//!     fc.set_client(true);
//!     let mut encoded = BytesMut::with_capacity(256);
//!
//!     if let Ok(Some(frame)) = fc.decode(&mut eb) {
//!         assert!(frame.fin());
//!         assert!(!frame.rsv1());
//!         assert!(!frame.rsv2());
//!         assert!(!frame.rsv3());
//!         // All frames from server must not be masked.
//!         assert!(!frame.masked());
//!         assert!(frame.opcode() == OpCode::Pong);
//!         assert!(frame.mask() == 0);
//!         assert!(frame.payload_length() == 0);
//!         assert!(frame.extension_data().is_none());
//!         assert!(frame.application_data().is_empty());
//!
//!         if fc.encode(frame, &mut encoded).is_ok() {
//!             for (a, b) in encoded.iter().zip(PONG.to_vec().iter()) {
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

// Client Only Codec Exports
pub use crate::codec::client::handshake::FrameCodec as HanshakeCodec;

// Common Frame Exports
pub use crate::frame::WebSocket as WebSocketFrame;
pub use crate::frame::base::{OpCode, Frame as BaseFrame};

// Client Only Frame Exports
pub use crate::frame::client::request::Frame as HandshakeRequestFrame;
pub use crate::frame::client::response::Frame as HandshakeResponseFrame;


//! Server API
//!
//! # Decode PING Base Frame from client
//!
//! ```
//! # extern crate tokio_core;
//! # extern crate twist;
//!
//! use twist::server::{BaseFrameCodec, OpCode};
//! use tokio_core::io::{Codec, EasyBuf};
//!
//! const PING: [u8; 6] = [0x89, 0x80, 0x00, 0x00, 0x00, 0x01];
//!
//! fn main() {
//!     let ping_vec = PING.to_vec();
//!     let mut eb = EasyBuf::from(ping_vec);
//!     let mut fc: BaseFrameCodec = Default::default();
//!     let mut encoded = Vec::new();
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
//!         assert!(frame.application_data().is_none());
//!
//!         if fc.encode(frame, &mut encoded).is_ok() {
//!             for (a, b) in encoded.iter().zip(PING.to_vec().iter()) {
//!                 assert!(a == b);
//!             }
//!         }
//!     } else {
//!         assert!(false);
//!     }
//! }
//! ```
// Common Codec Exports
pub use codec::Twist as TwistCodec;
pub use codec::base::FrameCodec as BaseFrameCodec;

// Server Only Codec Exports
pub use codec::server::handshake::FrameCodec as HandshakeCodec;

// Common Frame Exports
pub use frame::WebSocket as WebSocketFrame;
pub use frame::base::Frame as BaseFrame;
pub use frame::base::OpCode;

// Server Only Frame Exports
pub use frame::server::request::Frame as HandshakeRequestFrame;
pub use frame::server::response::Frame as HandshakeResponseFrame;

// Protocol Exports
pub use proto::server::WebSocketProtocol;

// Copyright (c) 2016 twist developers
//
// Licensed under the Apache License, Version 2.0
// <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0> or the MIT
// license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. All files in the project carrying such notice may not be copied,
// modified, or distributed except according to those terms.

//! An implementation of the [RFC6455][rfc6455] websocket protocol as
//! a set of tokio [`Codecs`][codec] and a tokio-proto pipeline [`ServerProto`][proto]
//!
//! # Decode Client Base Frame
//!
//! ```
//! # extern crate tokio_core;
//! # extern crate twist;
//!
//! use twist::{BaseFrameCodec, OpCode};
//! use tokio_core::io::{Codec, EasyBuf};
//!
//! const PING: [u8; 6] = [0x89, 0x80, 0x00, 0x00, 0x00, 0x01];
//!
//! fn main() {
//!     let ping_vec = PING.to_vec();
//!     let mut eb = EasyBuf::from(ping_vec);
//!     let mut fc: BaseFrameCodec = Default::default();
//!     fc.set_client(true);
//!     let mut encoded = Vec::new();
//!
//!     if let Ok(Some(frame)) = fc.decode(&mut eb) {
//!         assert!(frame.fin());
//!         assert!(!frame.rsv1());
//!         assert!(!frame.rsv2());
//!         assert!(!frame.rsv3());
//!         // All client frames must be masked.
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
//!
//! # Decode Server Base Frame
//!
//! ```
//! # extern crate tokio_core;
//! # extern crate twist;
//!
//! use twist::{BaseFrameCodec, OpCode};
//! use tokio_core::io::{Codec, EasyBuf};
//!
//! const PONG: [u8; 2] = [0x8a, 0x00];
//!
//! fn main() {
//!     let ping_vec = PONG.to_vec();
//!     let mut eb = EasyBuf::from(ping_vec);
//!     let mut fc: BaseFrameCodec = Default::default();
//!     let mut encoded = Vec::new();
//!
//!     if let Ok(Some(frame)) = fc.decode(&mut eb) {
//!         assert!(frame.fin());
//!         assert!(!frame.rsv1());
//!         assert!(!frame.rsv2());
//!         assert!(!frame.rsv3());
//!         // All server frames must not be masked.
//!         assert!(!frame.masked());
//!         assert!(frame.opcode() == OpCode::Pong);
//!         assert!(frame.mask() == 0);
//!         assert!(frame.payload_length() == 0);
//!         assert!(frame.extension_data().is_none());
//!         assert!(frame.application_data().is_none());
//!
//!         if fc.encode(frame, &mut encoded).is_ok() {
//!             for (a, b) in encoded.iter().zip(PONG.to_vec().iter()) {
//!                 assert!(a == b);
//!             }
//!         }
//!     } else {
//!         assert!(false);
//!     }
//! }
//! ```
//!
//! [rfc6455]: https://tools.ietf.org/html/rfc6455
//! [codec]: https://docs.rs/tokio-core/0.1.4/tokio_core/io/trait.Codec.html
//! [proto]: https://docs.rs/tokio-proto/0.1.0/tokio_proto/pipeline/trait.ServerProto.html
#![deny(missing_docs)]
extern crate base64;
extern crate byteorder;
extern crate httparse;
extern crate sha1;
extern crate tokio_core;
extern crate tokio_proto;
extern crate uuid;

#[macro_use]
extern crate futures;
#[macro_use]
extern crate slog;
#[macro_use]
mod macros;

mod codec;
mod ext;
mod frame;
mod proto;
mod util;

pub use codec::base::FrameCodec as BaseFrameCodec;
pub use codec::Twist as TwistCodec;
pub use codec::client::handshake::FrameCodec as ClientHanshakeCodec;
pub use codec::server::handshake::FrameCodec as ServerHandshakeCodec;
pub use ext::{FromHeader, IntoResponse, PerMessage, PerFrame};
pub use frame::WebSocket as WebSocketFrame;
pub use frame::base::Frame as BaseFrame;
pub use frame::base::OpCode;
pub use proto::WebSocketProtocol;

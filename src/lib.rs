// Copyright (c) 2016 twist developers
//
// Licensed under the Apache License, Version 2.0
// <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0> or the MIT
// license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. All files in the project carrying such notice may not be copied,
// modified, or distributed except according to those terms.

//! An implementation of the [RFC6455][rfc6455] websocket protocol as
//! a set of tokio [`Codec`][codec] anda tokio-proto pipeline [`ServerProto`][proto]
//!
//! # Basic Usage
//!
//! ```ignore
//! use twist::proto::Frame;
//! use tokio_proto::TcpServer;
//!
//! let ws_proto: Frame = Default::default();
//! let server = TcpServer::new(ws_proto, unenc_socket_addr);
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
extern crate slog_atomic;
extern crate slog_term;
extern crate tokio_core;
extern crate tokio_proto;

#[macro_use]
extern crate futures;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate slog;
#[macro_use]
mod macros;

mod codec;
mod ext;
mod frame;
mod proto;
mod util;

pub use codec::Twist as TwistCodec;
pub use codec::base::FrameCodec as BaseFrameCodec;
pub use codec::handshake::FrameCodec as HanshakeCodec;
pub use ext::{FromHeader, IntoResponse, PerMessage, PerFrame};
pub use frame::WebSocket as WebSocketFrame;
pub use frame::base::Frame as BaseFrame;
pub use frame::base::OpCode;
pub use proto::{set_stdout_level, WebSocketProtocol};

use std::io;
use std::sync::{Arc, Mutex};

#[cfg(feature = "ext")]
lazy_static! {
    static ref PM_EXTS: Arc<Mutex<Vec<Box<PerMessage>>>> = Arc::new(Mutex::new(Vec::new()));
    static ref PF_EXTS: Arc<Mutex<Vec<Box<PerFrame>>>> = Arc::new(Mutex::new(Vec::new()));
}

/// Register a per-message extension with this codec.
#[cfg(feature = "ext")]
pub fn register_pm_extension<T>(extension: T) -> Result<(), io::Error>
    where T: PerMessage + 'static
{
    let lock = PM_EXTS.clone();
    let mut vec = match lock.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    vec.push(Box::new(extension));
    Ok(())
}

/// Register a per-frame extension with this codec.
#[cfg(feature = "ext")]
pub fn register_pf_extension<T>(extension: T) -> Result<(), io::Error>
    where T: PerFrame + 'static
{
    let lock = PF_EXTS.clone();
    let mut vec = match lock.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    vec.push(Box::new(extension));
    Ok(())
}

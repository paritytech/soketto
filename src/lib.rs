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
//! [rfc6455]: https://tools.ietf.org/html/rfc6455
//! [codec]: https://docs.rs/tokio-core/0.1.4/tokio_core/io/trait.Codec.html
//! [proto]: https://docs.rs/tokio-proto/0.1.0/tokio_proto/pipeline/trait.ServerProto.html
#![deny(missing_docs)]
extern crate base64;
extern crate byteorder;
extern crate bytes;
extern crate encoding;
extern crate httparse;
extern crate rand;
extern crate sha1;
extern crate tokio_core;
extern crate tokio_io;
extern crate tokio_proto;
extern crate uuid;

#[macro_use]
extern crate futures;
#[macro_use]
extern crate nom;
#[macro_use]
extern crate slog;
#[macro_use]
mod macros;

pub mod client;
pub mod server;
pub mod extension;

mod codec;
mod frame;
mod proto;
mod util;

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

pub mod client;
pub mod server;
pub mod extension;

mod codec;
mod frame;
mod util;

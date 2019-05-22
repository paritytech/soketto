// Copyright (c) 2016 twist developers
//
// Licensed under the Apache License, Version 2.0
// <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0> or the MIT
// license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. All files in the project carrying such notice may not be copied,
// modified, or distributed except according to those terms.

//! An implementation of the [RFC6455][rfc6455] websocket protocol as a set
//! of tokio codecs.
//!
//! [rfc6455]: https://tools.ietf.org/html/rfc6455

pub mod base;
pub mod handshake;
pub mod connection;

pub use connection::{Connection, Mode};


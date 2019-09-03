// Copyright (c) 2019 Parity Technologies (UK) Ltd.
// Copyright (c) 2016 twist developers
//
// Licensed under the Apache License, Version 2.0
// <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0> or the MIT
// license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. All files in the project carrying such notice may not be copied,
// modified, or distributed except according to those terms.

//! An implementation of the [RFC6455][rfc6455] websocket protocol.
//!
//! [rfc6455]: https://tools.ietf.org/html/rfc6455

pub mod base;
pub mod extension;
pub mod handshake;
pub mod connection;

pub type BoxedError = Box<dyn std::error::Error + Send + Sync>;

/// A parsing result.
#[derive(Debug, Clone)]
pub enum Parsing<T, N = ()> {
    /// Parsing completed.
    Done {
        /// The parsed value.
        value: T,
        /// The offset into the byte slice that has been consumed.
        offset: usize
    },
    /// Parsing is incomplete and needs more data.
    NeedMore(N)
}

/// Helper function to allow casts from `usize` to `u64` only on platforms
/// where the sizes are guaranteed to fit.
#[cfg(any(target_pointer_width = "32", target_pointer_width = "64"))]
pub(crate) fn as_u64(a: usize) -> u64 {
    a as u64
}


// Copyright (c) 2016 twist developers
//
// Licensed under the Apache License, Version 2.0
// <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0> or the MIT
// license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. All files in the project carrying such notice may not be copied,
// modified, or distributed except according to those terms.

//! An implementation of the [RFC6455][rfc6455] websocket protocol as
//! a set of tokio codecs.
//!
//! [rfc6455]: https://tools.ietf.org/html/rfc6455

pub mod next;


pub mod codec;
pub mod frame;

/// A base64-encoded random value.
#[derive(Debug)]
pub struct Nonce(String);

impl Nonce {
    pub fn fresh() -> Self {
        use rand::Rng;
        let mut buf = [0; 16];
        rand::thread_rng().fill(&mut buf);
        Self(base64::encode(&buf))
    }

    pub(crate) fn wrap(s: String) -> Self {
        Nonce(s)
    }
}

impl AsRef<str> for Nonce {
    fn as_ref(&self) -> &str {
        &self.0
    }
}


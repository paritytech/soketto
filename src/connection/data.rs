// Copyright (c) 2019 Parity Technologies (UK) Ltd.
//
// Licensed under the Apache License, Version 2.0
// <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0> or the MIT
// license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. All files in the project carrying such notice may not be copied,
// modified, or distributed except according to those terms.

use bytes::BytesMut;

/// Websocket message payload data.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Data {
    /// Binary data.
    Binary(BytesMut),
    /// UTF-8 encoded data.
    Text(BytesMut)
}

impl Data {
    /// Is this binary data?
    pub fn is_binary(&self) -> bool {
        if let Data::Binary(_) = self { true } else { false }
    }

    /// Is this UTF-8 encoded textual data?
    pub fn is_text(&self) -> bool {
        if let Data::Text(_) = self { true } else { false }
    }

    /// The data lengths in bytes.
    pub fn len(&self) -> usize {
        self.as_ref().len()
    }

    /// Get a mutable reference to the inner bytes.
    ///
    /// Not public because a mutable reference to UTF-8 encoded text
    /// may be used to change it in ways that render the encoding
    /// invalid. Use with care!
    pub(crate) fn as_mut(&mut self) -> &mut BytesMut {
        match self {
            Data::Binary(d) => d,
            Data::Text(d) => d
        }
    }
}

impl AsRef<BytesMut> for Data {
    fn as_ref(&self) -> &BytesMut {
        match self {
            Data::Binary(d) => d,
            Data::Text(d) => d
        }
    }
}

impl Into<BytesMut> for Data {
    fn into(self) -> BytesMut {
        match self {
            Data::Binary(d) => d,
            Data::Text(d) => d
        }
    }
}

impl From<BytesMut> for Data {
    fn from(b: BytesMut) -> Self {
        Data::Binary(b)
    }
}

impl From<&'_ str> for Data {
    fn from(s: &str) -> Self {
        Data::Text(BytesMut::from(s))
    }
}

impl From<String> for Data {
    fn from(s: String) -> Self {
        Data::Text(BytesMut::from(s))
    }
}

impl From<&'_ [u8]> for Data {
    fn from(b: &[u8]) -> Self {
        Data::Binary(BytesMut::from(b))
    }
}

impl From<Vec<u8>> for Data {
    fn from(b: Vec<u8>) -> Self {
        Data::Binary(BytesMut::from(b))
    }
}


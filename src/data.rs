// Copyright (c) 2019 Parity Technologies (UK) Ltd.
//
// Licensed under the Apache License, Version 2.0
// <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0> or the MIT
// license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. All files in the project carrying such notice may not be copied,
// modified, or distributed except according to those terms.

//! The types of data to send or receive over a websocket connection.
//!
//! - [`Incoming`] data contains either normal application data such as text
//! or binary data or application data included in a PONG control frame.
//! Values of this type are produced when receiving data.
//!
//! - [`Outgoing`] data contains either normal application data such as text
//! or binary data or application data to include in a PING or PONG control
//! frame. Values of this type are used when sending data.
//!
//! - [`Data`] contains either textual or binary data.

use bytes::BytesMut;

/// Incoming data.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Incoming {
    /// Text or binary data.
    Data(Data),
    /// Data sent with a PONG control frame.
    Pong(BytesMut)
}

impl Incoming {
    /// Is this text or binary data?
    pub fn is_data(&self) -> bool {
        if let Incoming::Data(_) = self { true } else { false }
    }

    /// Is this a PONG?
    pub fn is_pong(&self) -> bool {
        if let Incoming::Pong(_) = self { true } else { false }
    }

    /// The data length in bytes.
    pub fn len(&self) -> usize {
        self.as_ref().len()
    }
}

impl AsRef<BytesMut> for Incoming {
    fn as_ref(&self) -> &BytesMut {
        match self {
            Incoming::Data(d) => d.as_ref(),
            Incoming::Pong(d) => d
        }
    }
}

impl AsMut<BytesMut> for Incoming {
    fn as_mut(&mut self) -> &mut BytesMut {
        match self {
            Incoming::Data(d) => d.as_mut(),
            Incoming::Pong(d) => d
        }
    }
}

impl From<Data> for Incoming {
    fn from(d: Data) -> Self {
        Incoming::Data(d)
    }
}

/// Outgoing data.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Outgoing {
    /// Text or binary data.
    Data(Data),
    /// Data to include in a PING control frame.
    Ping(BytesMut),
    /// Data to include in a PONG control frame.
    Pong(BytesMut)
}

impl Outgoing {
    /// Is this text or binary data?
    pub fn is_data(&self) -> bool {
        if let Outgoing::Data(_) = self { true } else { false }
    }

    /// Is this a PING?
    pub fn is_ping(&self) -> bool {
        if let Outgoing::Ping(_) = self { true } else { false }
    }

    /// Is this a PONG?
    pub fn is_pong(&self) -> bool {
        if let Outgoing::Pong(_) = self { true } else { false }
    }

    /// The data length in bytes.
    pub fn len(&self) -> usize {
        self.as_ref().len()
    }
}

impl AsRef<BytesMut> for Outgoing {
    fn as_ref(&self) -> &BytesMut {
        match self {
            Outgoing::Data(d) => d.as_ref(),
            Outgoing::Ping(d) => d,
            Outgoing::Pong(d) => d
        }
    }
}

impl AsMut<BytesMut> for Outgoing {
    fn as_mut(&mut self) -> &mut BytesMut {
        match self {
            Outgoing::Data(d) => d.as_mut(),
            Outgoing::Ping(d) => d,
            Outgoing::Pong(d) => d
        }
    }
}

impl From<Data> for Outgoing {
    fn from(d: Data) -> Self {
        Outgoing::Data(d)
    }
}

/// Payload application data.
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

    /// The data length in bytes.
    pub fn len(&self) -> usize {
        self.as_ref().len()
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

impl AsMut<BytesMut> for Data {
    fn as_mut(&mut self) -> &mut BytesMut {
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


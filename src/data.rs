// Copyright (c) 2019 Parity Technologies (UK) Ltd.
//
// Licensed under the Apache License, Version 2.0
// <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0> or the MIT
// license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. All files in the project carrying such notice may not be copied,
// modified, or distributed except according to those terms.

use bytes::BytesMut;
use std::convert::TryFrom;

/// Data received from the remote end.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Incoming {
    /// Text or binary data.
    Data(Data),
    /// Data sent with a PONG control frame.
    Pong(Data)
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

    /// Is this text data?
    pub fn is_text(&self) -> bool {
        if let Incoming::Data(d) = self {
            d.is_text()
        } else {
            false
        }
    }

    /// Is this binary data?
    pub fn is_binary(&self) -> bool {
        if let Incoming::Data(d) = self {
            d.is_binary()
        } else {
            false
        }
    }
}

impl AsRef<[u8]> for Incoming {
    fn as_ref(&self) -> &[u8] {
        match self {
            Incoming::Data(d) => d.as_ref(),
            Incoming::Pong(d) => d.as_ref()
        }
    }
}

impl AsMut<[u8]> for Incoming {
    fn as_mut(&mut self) -> &mut [u8] {
        match self {
            Incoming::Data(d) => d.as_mut(),
            Incoming::Pong(d) => d.as_mut()
        }
    }
}

impl Into<Data> for Incoming {
    fn into(self: Incoming) -> Data {
        match self {
            Incoming::Data(d) => d,
            Incoming::Pong(d) => d
        }
    }
}

/// Application data.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Data(DataRepr);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum DataRepr {
    /// Binary data.
    Binary(BytesMut),
    /// UTF-8 encoded data.
    Text(BytesMut)
}

impl Data {
    /// Create a new binary `Data` value.
    pub(crate) fn binary(b: BytesMut) -> Self {
        Data(DataRepr::Binary(b))
    }

    /// Create a new textual `Data` value.
    pub(crate) fn text(b: BytesMut) -> Self {
        debug_assert!(std::str::from_utf8(&b).is_ok());
        Data(DataRepr::Text(b))
    }

    /// Is this binary data?
    pub fn is_binary(&self) -> bool {
        if let DataRepr::Binary(_) = self.0 { true } else { false }
    }

    /// Is this UTF-8 encoded textual data?
    pub fn is_text(&self) -> bool {
        if let DataRepr::Text(_) = self.0 { true } else { false }
    }
}

impl AsRef<[u8]> for Data {
    fn as_ref(&self) -> &[u8] {
        match &self.0 {
            DataRepr::Binary(d) => d,
            DataRepr::Text(d) => d
        }
    }
}

impl AsMut<[u8]> for Data {
    fn as_mut(&mut self) -> &mut [u8] {
        match &mut self.0 {
            DataRepr::Binary(d) => d,
            DataRepr::Text(d) => d
        }
    }
}

/// Wrapper which restricts the length of its byte slice to 125 bytes.
#[derive(Debug)]
pub struct ByteSlice125<'a>(&'a [u8]);

static_assertions::const_assert_eq!(125, crate::base::MAX_CTRL_BODY_SIZE);

/// Error, if converting to `[ByteSlice125`] fails.
#[derive(Clone, Debug, thiserror::Error)]
#[error("Slice larger than 125 bytes")]
pub struct SliceTooLarge(());

impl<'a> TryFrom<&'a [u8]> for ByteSlice125<'a> {
    type Error = SliceTooLarge;

    fn try_from(value: &'a [u8]) -> Result<Self, Self::Error> {
        if value.len() > 125 {
            Err(SliceTooLarge(()))
        } else {
            Ok(ByteSlice125(value))
        }
    }
}

impl AsRef<[u8]> for ByteSlice125<'_> {
    fn as_ref(&self) -> &[u8] {
        self.0
    }
}


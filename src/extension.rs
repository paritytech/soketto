// Copyright (c) 2019 Parity Technologies (UK) Ltd.
// Copyright (c) 2016 twist developers
//
// Licensed under the Apache License, Version 2.0
// <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0> or the MIT
// license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. All files in the project carrying such notice may not be copied,
// modified, or distributed except according to those terms.

use crate::base::{Frame, OpCode};
use std::{borrow::Cow, error::Error};

/// A websocket extension as per RFC 6455, section 9.
pub trait Extension: std::fmt::Debug {
    /// Is this extension enabled?
    fn is_enabled(&self) -> bool;

    /// The name of this extension.
    fn name(&self) -> &str;

    /// The parameters this extension wants to send for negotiation.
    fn params(&self) -> &[Param];

    /// Configure this extension with the parameters received from negotiation.
    fn configure(&mut self, params: &[Param]) -> Result<(), Box<dyn Error + Send>>;

    /// Encode the given frame.
    fn encode(&mut self, f: &mut Frame) -> Result<(), Box<dyn Error + Send>>;

    /// Decode the given frame.
    fn decode(&mut self, f: &mut Frame) -> Result<(), Box<dyn Error + Send>>;

    /// The reserved bits this extension uses.
    fn reserved_bits(&self) -> (bool, bool, bool) {
        (false, false, false)
    }

    /// The reserved opcode of this extension (must be one of `OpCode::Reserved*`).
    fn reserved_opcode(&self) -> Option<OpCode> {
        None
    }
}

impl<E: Extension + ?Sized> Extension for Box<E> {
    fn is_enabled(&self) -> bool {
        (**self).is_enabled()
    }

    fn name(&self) -> &str {
        (**self).name()
    }

    fn params(&self) -> &[Param] {
        (**self).params()
    }

    fn configure(&mut self, params: &[Param]) -> Result<(), Box<dyn Error + Send>> {
        (**self).configure(params)
    }

    fn encode(&mut self, f: &mut Frame) -> Result<(), Box<dyn Error + Send>> {
        (**self).encode(f)
    }

    fn decode(&mut self, f: &mut Frame) -> Result<(), Box<dyn Error + Send>> {
        (**self).decode(f)
    }

    fn reserved_bits(&self) -> (bool, bool, bool) {
        (**self).reserved_bits()
    }

    fn reserved_opcode(&self) -> Option<OpCode> {
        (**self).reserved_opcode()
    }
}

/// Extension parameter (used for negotiation).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Param<'a> {
    pub(crate) name: Cow<'a, str>,
    pub(crate) value: Option<Cow<'a, str>>
}

impl<'a> Param<'a> {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn value(&self) -> Option<&str> {
        self.value.as_ref().map(|v| v.as_ref())
    }
}


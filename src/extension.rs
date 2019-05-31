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
    /// The name of this extension.
    fn name(&self) -> &str;

    /// The scope this extension applies to.
    fn scope(&self) -> Scope;

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
    fn name(&self) -> &str {
        (**self).name()
    }

    fn scope(&self) -> Scope {
        (**self).scope()
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
#[derive(Debug, Clone)]
pub struct Param<'a> {
    pub(crate) name: Cow<'a, str>,
    pub(crate) value: Option<Cow<'a, [u8]>>
}

impl<'a> Param<'a> {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn value(&self) -> Option<&[u8]> {
        self.value.as_ref().map(|v| v.as_ref())
    }
}

/// The scope an extension applies to, i.e. when it should be invoked.
#[derive(Debug, Clone)]
pub enum Scope {
    /// Frame and Message scope.
    All,
    /// Frame scope.
    Frame,
    /// Message scope, i.e. after fragmented frames have been processed.
    Message,
}

pub(crate) fn reserved_bits_union<'a, I, E>(exts: I) -> (bool, bool, bool)
where
    I: Iterator<Item = &'a E>,
    E: Extension + 'a
{
    let mut union = (false, false, false);
    for e in exts {
        let (r1, r2, r3) = e.reserved_bits();
        union.0 |= r1;
        union.1 |= r2;
        union.2 |= r3
    }
    union
}

//! A websocket [base](https://tools.ietf.org/html/rfc6455#section-5.2) frame

use bytes::BytesMut;
use std::{convert::TryFrom, fmt};

/// Operation codes defined in [RFC6455](https://tools.ietf.org/html/rfc6455#section-5.2).
#[derive(Debug, Eq, PartialEq, Clone, Copy)]
pub enum OpCode {
    /// Indicates a continuation frame of a fragmented message.
    Continue,
    /// Indicates a text data frame.
    Text,
    /// Indicates a binary data frame.
    Binary,
    /// Indicates a close control frame.
    Close,
    /// Indicates a ping control frame.
    Ping,
    /// Indicates a pong control frame.
    Pong,
    /// Indicates a reserved op code.
    Reserved
}

impl OpCode {
    /// Is this a control opcode?
    pub fn is_control(self) -> bool {
        match self {
            OpCode::Close | OpCode::Ping | OpCode::Pong => true,
            _ => false
        }
    }

    /// Is this opcode reserved?
    pub fn is_reserved(self) -> bool {
        match self {
            OpCode::Reserved => true,
            _ => false
        }
    }
}

impl fmt::Display for OpCode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            OpCode::Continue => f.write_str("Continue"),
            OpCode::Text => f.write_str("Text"),
            OpCode::Binary => f.write_str("Binary"),
            OpCode::Close => f.write_str("Close"),
            OpCode::Ping => f.write_str("Ping"),
            OpCode::Pong => f.write_str("Pong"),
            OpCode::Reserved => f.write_str("Reserved")
        }
    }
}

#[derive(Debug)]
pub struct UnknownOpCode(());

impl fmt::Display for UnknownOpCode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("unknown opcode")
    }
}

impl std::error::Error for UnknownOpCode {}

impl TryFrom<u8> for OpCode {
    type Error = UnknownOpCode;

    fn try_from(val: u8) -> Result<OpCode, Self::Error> {
        match val {
            0 => Ok(OpCode::Continue),
            1 => Ok(OpCode::Text),
            2 => Ok(OpCode::Binary),
            8 => Ok(OpCode::Close),
            9 => Ok(OpCode::Ping),
            10 => Ok(OpCode::Pong),
            3 ... 7 | 11 ... 15 => Ok(OpCode::Reserved),
            _ => Err(UnknownOpCode(()))
        }
    }
}

impl From<OpCode> for u8 {
    fn from(opcode: OpCode) -> u8 {
        match opcode {
            OpCode::Continue => 0,
            OpCode::Text => 1,
            OpCode::Binary => 2,
            OpCode::Close => 8,
            OpCode::Ping => 9,
            OpCode::Pong => 10,
            OpCode::Reserved => 3
        }
    }
}

#[derive(Debug, Clone)]
pub struct Header {
    /// The `fin` flag.
    fin: bool,
    /// The `rsv1` flag.
    rsv1: bool,
    /// The `rsv2` flag.
    rsv2: bool,
    /// The `rsv3` flag.
    rsv3: bool,
    /// The 'mask' flag.
    masked: bool,
    /// The `opcode`
    opcode: OpCode,
    /// The `mask`.
    mask: u32
}

impl Header {
    pub fn new(oc: OpCode) -> Self {
        Self {
            fin: false,
            rsv1: false,
            rsv2: false,
            rsv3: false,
            masked: false,
            opcode: oc,
            mask: 0
        }
    }

    /// Get the `fin` flag.
    pub fn is_fin(&self) -> bool {
        self.fin
    }

    /// Set the `fin` flag.
    pub fn set_fin(&mut self, fin: bool) -> &mut Self {
        self.fin = fin;
        self
    }

    /// Get the `rsv1` flag.
    pub fn is_rsv1(&self) -> bool {
        self.rsv1
    }

    /// Set the `rsv1` flag.
    pub fn set_rsv1(&mut self, rsv1: bool) -> &mut Self {
        self.rsv1 = rsv1;
        self
    }

    /// Get the `rsv2` flag.
    pub fn is_rsv2(&self) -> bool {
        self.rsv2
    }

    /// Set the `rsv2` flag.
    pub fn set_rsv2(&mut self, rsv2: bool) -> &mut Self {
        self.rsv2 = rsv2;
        self
    }

    /// Get the `rsv3` flag.
    pub fn is_rsv3(&self) -> bool {
        self.rsv3
    }

    /// Set the `rsv3` flag.
    pub fn set_rsv3(&mut self, rsv3: bool) -> &mut Self {
        self.rsv3 = rsv3;
        self
    }

    /// Get the `masked` flag.
    pub fn is_masked(&self) -> bool {
        self.masked
    }

    /// Set the `masked` flag.
    pub fn set_masked(&mut self, masked: bool) -> &mut Self {
        self.masked = masked;
        self
    }

    /// Get the `opcode`.
    pub fn opcode(&self) -> OpCode {
        self.opcode
    }

    /// Set the `opcode`
    pub fn set_opcode(&mut self, opcode: OpCode) -> &mut Self {
        self.opcode = opcode;
        self
    }

    /// Get the `mask`.
    pub fn mask(&self) -> u32 {
        self.mask
    }

    /// Set the `mask`
    pub fn set_mask(&mut self, mask: u32) -> &mut Self {
        self.mask = mask;
        self
    }
}

impl From<Header> for Frame {
    fn from(header: Header) -> Self {
        Frame {
            header,
            extension_data: BytesMut::new(),
            application_data: BytesMut::new()
        }
    }
}

/// Represents the parts of a [base](https://tools.ietf.org/html/rfc6455#section-5.2) frame.
#[derive(Debug, Clone)]
pub struct Frame {
    /// The frame header.
    header: Header,
    /// The optional extension data.
    extension_data: BytesMut,
    /// The optional application data.
    application_data: BytesMut
}

impl Frame {
    /// Get the frame header.
    pub fn header(&self) -> &Header {
        &self.header
    }

    /// Get the `extension_data`.
    pub fn extension_data(&self) -> &[u8] {
        &self.extension_data
    }

    /// Set the `extension_data`.
    pub fn set_extension_data(&mut self, bytes: impl Into<BytesMut>) -> &mut Self {
        self.extension_data = bytes.into();
        self
    }

    /// Get the `application_data`
    pub fn application_data(&self) -> &[u8] {
        &self.application_data
    }

    /// Set the `application_data`
    pub fn set_application_data(&mut self, bytes: impl Into<BytesMut>) -> &mut Self {
        self.application_data = bytes.into();
        self
    }
}


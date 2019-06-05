// Copyright (c) 2019 Parity Technologies (UK) Ltd.
//
// Licensed under the Apache License, Version 2.0
// <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0> or the MIT
// license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. All files in the project carrying such notice may not be copied,
// modified, or distributed except according to those terms.

use crate::{base::{Data, Frame, OpCode}, extension::{Extension, Param}};
use log::trace;
use smallvec::SmallVec;
use std::{fmt, io};

#[derive(Debug)]
pub struct Deflate {
    enabled: bool,
    params: SmallVec<[Param<'static>; 2]>
}

impl Deflate {
    pub fn new() -> Self {
        Deflate {
            enabled: false,
            params: SmallVec::new()
        }
    }
}

impl Extension for Deflate {
    fn name(&self) -> &str {
        "permessage-deflate"
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }

    fn params(&self) -> &[Param] {
        &self.params
    }

    fn configure(&mut self, params: &[Param]) -> Result<(), crate::BoxError> {
        for p in params {
            match p.name() {
                "client_no_context_takeover" => {
                    // Not necessary to include a response.
                }
                "client_max_window_bits" => {
                    // Not necessary to include a response.
                }
                "server_no_context_takeover" => {
                    self.params.push(Param::new("server_no_context_takeover"))
                }
                "server_max_window_bits" => {
                    let mut x = Param::new("server_max_window_bits");
                    x.set_value(p.value().map(String::from));
                    self.params.push(x)
                }
                _ => return Err(Error::UnknownParameter(p.clone().acquire()).into())
            }
        }
        self.enabled = true;
        Ok(())
    }

    fn reserved_bits(&self) -> (bool, bool, bool) {
        (true, false, false)
    }

    fn decode(&mut self, f: &mut Frame) -> Result<(), crate::BoxError> {
        if !f.is_rsv1() {
            return Ok(())
        }
        if let Some(bytes) = f.payload_data_mut().map(|d| d.bytes_mut()) {
            trace!("to decode: {:02x?}", &bytes[..]);
            bytes.extend_from_slice(&[0, 0, 0xFF, 0xFF]);
            let mut d = flate2::Decompress::new(false);
            let mut v = Vec::with_capacity(bytes.len() * 2);
            d.decompress_vec(bytes, &mut v, flate2::FlushDecompress::Sync)?;
            match f.opcode() {
                OpCode::Text => {
                    trace!("decoded text: {:?}", std::str::from_utf8(&v));
                    f.set_payload_data(Some(Data::Text(v.into())));
                }
                OpCode::Binary => {
                    f.set_payload_data(Some(Data::Binary(v.into())));
                }
                _ => unreachable!()
            }
            f.set_rsv1(false);
        }
        Ok(())
    }

    fn encode(&mut self, f: &mut Frame) -> Result<(), crate::BoxError> {
        if let OpCode::Text | OpCode::Binary = f.opcode() {
            // ok
        } else {
            return Ok(())
        }
        if let Some(bytes) = f.payload_data().map(|d| d.as_ref()) {
            if f.opcode() == OpCode::Text {
                trace!("to encode: {:?}", std::str::from_utf8(bytes));
            } else {
                trace!("to encode: {:02x?}", &bytes[..]);
            }
            let mut d = flate2::Compress::new(flate2::Compression::fast(), false);
            let mut v = Vec::with_capacity(bytes.len() * 2);
            d.compress_vec(bytes, &mut v, flate2::FlushCompress::Sync)?;
            let n = v.len() - 4;
            v.truncate(n);
            trace!("encoded: {:02x?}", v);
            match f.opcode() {
                OpCode::Text => {
                    f.set_payload_data(Some(Data::Text(v.into())));
                }
                OpCode::Binary => {
                    f.set_payload_data(Some(Data::Binary(v.into())));
                }
                _ => unreachable!()
            }
            f.set_rsv1(true);
        }
        Ok(())
    }
}

// Error //////////////////////////////////////////////////////////////////////////////////////////

/// Enumeration of possible deflate errors.
#[derive(Debug)]
pub enum Error {
    /// An I/O error has been encountered.
    Io(io::Error),
    UnknownParameter(Param<'static>),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::Io(e) => write!(f, "i/o error: {}", e),
            Error::UnknownParameter(p) => write!(f, "unknown parameter: {}", p),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io(e) => Some(e),
            Error::UnknownParameter(_) => None
        }
    }
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Error::Io(e)
    }
}


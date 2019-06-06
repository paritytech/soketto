// Copyright (c) 2019 Parity Technologies (UK) Ltd.
//
// Licensed under the Apache License, Version 2.0
// <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0> or the MIT
// license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. All files in the project carrying such notice may not be copied,
// modified, or distributed except according to those terms.

use crate::{
    base::{Data, Header, OpCode},
    connection::Mode,
    extension::{Extension, Param}
};
use flate2::{Compress, Compression, Decompress, FlushCompress, FlushDecompress};
use log::debug;
use smallvec::SmallVec;

#[derive(Debug)]
pub struct Deflate {
    mode: Mode,
    enabled: bool,
    buffer: Vec<u8>,
    client_params: SmallVec<[Param<'static>; 2]>,
    server_params: SmallVec<[Param<'static>; 2]>,
    client_max_window_bits: u8,
    server_max_window_bits: u8
}

impl Deflate {
    pub fn new(mode: Mode) -> Self {
        let client_params = match mode {
            Mode::Server => SmallVec::new(),
            Mode::Client => {
                let mut params = SmallVec::new();
                params.push(Param::new("server_no_context_takeover"));
                params
            }
        };
        Deflate {
            mode,
            enabled: false,
            buffer: Vec::new(),
            client_params,
            server_params: SmallVec::new(),
            client_max_window_bits: 15,
            server_max_window_bits: 15
        }
    }

//// Not supported yet:
//    pub fn set_max_server_bits(&mut self, max: u8) -> &mut Self {
//        assert!(max >= 8 && max <= 15, "max. window bits have to be within [8, 15]");
//        self.server_max_window_bits = max;
//        self
//    }
//
//    pub fn set_max_client_bits(&mut self, max: u8) -> &mut Self {
//        assert!(max >= 8 && max <= 15, "max. window bits have to be within [8, 15]");
//        self.client_max_window_bits = max;
//        self
//    }
}

impl Extension for Deflate {
    fn name(&self) -> &str {
        "permessage-deflate"
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }

    fn params(&self) -> &[Param] {
        match self.mode {
            Mode::Client => &self.client_params,
            Mode::Server => &self.server_params
        }
    }

    fn configure(&mut self, params: &[Param]) -> Result<(), crate::BoxError> {
        self.enabled = false;
        match self.mode {
            Mode::Server => {
                self.server_params.clear();
                for p in params {
                    match p.name() {
                        "client_max_window_bits" => {} // Not necessary to include a response.
                        "server_max_window_bits" => {
                            if let Some(Ok(v)) = p.value().map(|s| s.parse::<u8>()) {
                                //// Once we support window bits < 15 this can be done:
                                // if v < 8 || v > 15 {
                                if v != 15 {
                                    debug!("unacceptable server_max_window_bits: {:?}", v);
                                    return Ok(())
                                }
                                let mut x = Param::new("server_max_window_bits");
                                x.set_value(Some(v.to_string()));
                                self.server_params.push(x);
                                self.server_max_window_bits = v;
                            } else {
                                debug!("invalid server_max_window_bits: {:?}", p.value());
                                return Ok(())
                            }
                        }
                        "client_no_context_takeover" =>
                            self.server_params.push(Param::new("client_no_context_takeover")),
                        "server_no_context_takeover" =>
                            self.server_params.push(Param::new("server_no_context_takeover")),
                        _ => {
                            debug!("{}: unknown parameter: {}", self.name(), p.name());
                            return Ok(())
                        }
                    }
                }
            }
            Mode::Client => {
                let mut server_no_context_takeover = false;
                for p in params {
                    match p.name() {
                        "server_no_context_takeover" => server_no_context_takeover = true,
                        "server_max_window_bits" => {}
                        "client_no_context_takeover" => {} // must be supported
                        _ => {
                            debug!("{}: unknown parameter: {}", self.name(), p.name());
                            return Ok(())
                        }
                    }
                }
                if !server_no_context_takeover {
                    debug!("{}: server did not confirm no context takeover", self.name());
                    return Ok(())
                }
            }
        }
        self.enabled = true;
        Ok(())
    }

    fn reserved_bits(&self) -> (bool, bool, bool) {
        (true, false, false)
    }

    fn decode(&mut self, hdr: &mut Header, data: &mut Option<Data>) -> Result<(), crate::BoxError> {
        match hdr.opcode() {
            OpCode::Binary | OpCode::Text if hdr.is_rsv1() && hdr.is_fin() => {}
            OpCode::Continue if hdr.is_fin() => {}
            _ => return Ok(())
        }
        if let Some(data) = data {
            data.bytes_mut().extend_from_slice(&[0, 0, 0xFF, 0xFF]); // cf. RFC 7692, section 7.2.2
            self.buffer.clear();
            let mut d = Decompress::new(false);
            while (d.total_in() as usize) < data.as_ref().len() {
                let off = d.total_in() as usize;
                self.buffer.reserve(data.as_ref().len() - off);
                d.decompress_vec(&data.as_ref()[off ..], &mut self.buffer, FlushDecompress::Sync)?;
            }
            data.bytes_mut().clear();
            data.bytes_mut().extend_from_slice(&self.buffer);
            hdr.set_rsv1(false);
        }
        Ok(())
    }

    fn encode(&mut self, hdr: &mut Header, data: &mut Option<Data>) -> Result<(), crate::BoxError> {
        match hdr.opcode() {
            OpCode::Text | OpCode::Binary => {},
            _ => return Ok(())
        }
        if let Some(data) = data {
            let mut c = Compress::new(Compression::fast(), false);
            self.buffer.clear();
            while (c.total_in() as usize) < data.as_ref().len() {
                let off = c.total_in() as usize;
                self.buffer.reserve(data.as_ref().len() - off);
                c.compress_vec(&data.as_ref()[off ..], &mut self.buffer, FlushCompress::Sync)?;
            }
            if self.buffer.capacity() - self.buffer.len() < 5 {
                self.buffer.reserve(5); // Make room for the trailing end bytes
                c.compress_vec(&[], &mut self.buffer, FlushCompress::Sync)?;
            }
            let n = self.buffer.len() - 4;
            self.buffer.truncate(n); // Remove [0, 0, 0xFF, 0xFF]; cf. RFC 7692, section 7.2.1
            data.bytes_mut().clear();
            data.bytes_mut().extend_from_slice(&self.buffer);
            hdr.set_rsv1(true);
        }
        Ok(())
    }
}


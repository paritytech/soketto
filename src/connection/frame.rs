// Copyright (c) 2019 Parity Technologies (UK) Ltd.
//
// Licensed under the Apache License, Version 2.0
// <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0> or the MIT
// license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. All files in the project carrying such notice may not be copied,
// modified, or distributed except according to those terms.

//! Define a websocket message frame type and accompanying codec.

use bytes::BytesMut;
use crate::{base::{self, Header, OpCode}, Parsing};
use super::Error;

/// Websocket header and payload data pair.
#[derive(Debug)]
pub(crate) struct Frame {
    header: Header,
    payload: BytesMut
}

impl Frame {
    pub(crate) fn new(mut h: Header, p: BytesMut) -> Self {
        h.set_payload_len(p.len());
        Frame { header: h, payload: p }
    }

    pub(crate) fn is_text(&self) -> bool {
        self.header.opcode() == OpCode::Text
    }

    pub(crate) fn header(&self) -> &Header {
        &self.header
    }

    pub(crate) fn header_mut(&mut self) -> &mut Header {
        &mut self.header
    }

    pub(crate) fn payload(&self) -> &[u8] {
        &self.payload
    }

    pub(crate) fn set_payload(&mut self, p: BytesMut) {
        self.header.set_payload_len(p.len());
        self.payload = p;
    }

    pub(crate) fn into(self) -> (Header, BytesMut) {
        (self.header, self.payload)
    }

    pub(crate) fn into_payload(self) -> BytesMut {
        self.payload
    }

    pub(crate) fn as_mut(&mut self) -> (&mut Header, &mut BytesMut) {
        (&mut self.header, &mut self.payload)
    }
}


/// Encode and decode `Frame` values.
#[derive(Debug)]
pub(crate) struct FrameCodec {
    pub(crate) codec: base::Codec,
    pub(crate) header: Option<Header>
}

impl tokio_codec::Decoder for FrameCodec {
    type Item = Frame;
    type Error = Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if self.header.is_none() {
            if let Parsing::Done { value: header, offset } = self.codec.decode_header(&src)? {
                src.split_to(offset);
                self.header = Some(header)
            } else {
                return Ok(None)
            }
        }

        let mut header = self.header.take()
            .expect("header is_some() because we have parsed it above or it was already is_some()");

        if src.len() < header.payload_len() {
            src.reserve(header.payload_len() - src.len());
            self.header = Some(header);
            return Ok(None)
        }

        let mut payload = src.split_to(header.payload_len());
        base::Codec::apply_mask(&header, &mut payload);
        header.set_masked(false);
        header.set_mask(0);

        Ok(Some(Frame { header, payload }))
    }
}

impl tokio_codec::Encoder for FrameCodec {
    type Item = Frame;
    type Error = Error;

    fn encode(&mut self, frame: Self::Item, dst: &mut BytesMut) -> Result<(), Self::Error> {
        let bytes = self.codec.encode_header(&frame.header);
        dst.extend_from_slice(bytes);
        dst.extend_from_slice(&frame.payload);
        Ok(())
    }
}


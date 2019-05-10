//! Codec for decoding/encoding websocket client handshake frames.

use bytes::BytesMut;
use crate::codec::http::{self, ResponseHeaderCodec};
use crate::frame::handshake;
use tokio_io::codec::Decoder;
use crate::util::{self, Nonce};

/// Decoder of websocket server to client handshake response.
#[derive(Debug)]
pub struct FrameCodec<'a> {
    nonce: &'a Nonce
}

impl<'a> FrameCodec<'a> {
    pub fn new(nonce: &'a Nonce) -> Self {
        Self { nonce }
    }
}

impl<'a> Decoder for FrameCodec<'a> {
    type Item = handshake::client::Response;
    type Error = http::Error;

    fn decode(&mut self, buf: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if let Some(res) = ResponseHeaderCodec::new().decode(buf)? {
            match handshake::client::Response::new(self.nonce, res) {
                Ok(handshake) => Ok(Some(handshake)),
                Err(invalid) => unimplemented!()
            }
        } else {
            Ok(None)
        }
    }
}

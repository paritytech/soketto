//! Codec for dedoding/encoding websocket client handshake frames.
use frame::client::handshake::Frame;
use std::io;
use tokio_core::io::{Codec, EasyBuf};

/// Codec for decoding/encoding websocket client handshake frames.
#[derive(Default)]
pub struct FrameCodec;

impl Codec for FrameCodec {
    type In = Frame;
    type Out = Frame;

    fn decode(&mut self, _buf: &mut EasyBuf) -> Result<Option<Self::In>, io::Error> {
        Ok(None)
    }

    fn encode(&mut self, _msg: Self::Out, _buf: &mut Vec<u8>) -> io::Result<()> {
        Ok(())
    }
}

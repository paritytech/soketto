//! Codec for dedoding/encoding websocket server handshake frames.
use bytes::BytesMut;
use crate::frame::server::request::{ClientHandshake, Validated};
use crate::frame::server::response::Frame as ServerResponse;
use crate::codec::http::{self, RequestHeaderCodec, ResponseHeaderCodec};
use crate::util;
use log::trace;
use httparse::{EMPTY_HEADER, Request};
use std::{collections::HashMap, io};
use tokio_io::codec::{Decoder, Encoder};

/// Codec for decoding/encoding websocket server handshake frames.
#[derive(Debug, Default)]
pub struct FrameCodec(());

impl Decoder for FrameCodec {
    type Item = ClientHandshake<Validated>;
    type Error = http::Error;

    fn decode(&mut self, buf: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if let Some(req) = RequestHeaderCodec::new().decode(buf)? {
            match ClientHandshake::new(req).validated() {
                Ok(handshake) => Ok(Some(handshake)),
                Err(invalid) => unimplemented!()
            }
        } else {
            Ok(None)
        }
    }
}

impl Encoder for FrameCodec {
    type Item = ServerResponse;
    type Error = io::Error;

    fn encode(&mut self, msg: Self::Item, buf: &mut BytesMut) -> Result<(), Self::Error> {
        let code = msg.code();
        let mut response = format!("HTTP/1.1 {} {}\r\n", code, msg.reason());

        if let 101 = code {
            response.push_str("Upgrade: websocket\r\n");
            response.push_str("Connection: upgrade\r\n");
            response.push_str(&format!("Sec-WebSocket-Accept: {}\r\n", msg.accept_val()?));

//            if let Some(ref ext_resp) = self.ext_resp {
//                if !ext_resp.is_empty() {
//                    response.push_str(ext_resp);
//                    response.push_str("\r\n");
//                }
//            }
        }

        // Add the other headers to the response.
        for (k, v) in msg.others().iter() {
            response.push_str(&format!("{}: {}\r\n", *k, *v));
        }

        response.push_str("\r\n");

        trace!("handshake response\n{}", response);
        buf.extend(response.as_bytes());
        Ok(())
    }
}

// #[cfg(test)]
// mod test {
//     use super::FrameCodec;
//
//     #[test]
//     pub fn accept() {
//         let hf: FrameCodec = Default::default();
//         if let Ok(res) = hf.accept_val("dGhlIHNhbXBsZSBub25jZQ==".to_string()) {
//             assert!(res == "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=");
//         } else {
//             assert!(false);
//         }
//     }
// }

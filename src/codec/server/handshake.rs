//! Codec for dedoding/encoding websocket server handshake frames.
use bytes::BytesMut;
use crate::frame::server::request::Frame as ClientRequest;
use crate::frame::server::response::Frame as ServerResponse;
use crate::util;
use log::trace;
use httparse::{EMPTY_HEADER, Request};
use std::{collections::HashMap, io};
use tokio_io::codec::{Decoder, Encoder};

#[derive(Default)]
/// Codec for decoding/encoding websocket server handshake frames.
pub struct FrameCodec {
    /// Extension Negotiation Response.
    ext_resp: Option<String>
}

impl FrameCodec {
    /// Set the extension negotiation response.
    pub fn set_ext_resp(&mut self, response: &str) -> &mut FrameCodec {
        self.ext_resp = Some(String::from(response));
        self
    }
}

impl Decoder for FrameCodec {
    type Item = ClientRequest;
    type Error = io::Error;
    // type Out = ServerResponse;

    fn decode(&mut self, buf: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        let len = buf.len();
        if len == 0 {
            return Ok(None);
        }
        let drained = buf.split_to(len);
        let req_bytes = &drained;
        let mut headers = [EMPTY_HEADER; 32];
        let mut req = Request::new(&mut headers);
        let mut handshake_frame: ClientRequest = Default::default();

        if let Ok(res) = req.parse(req_bytes) {
            if res.is_complete() {
                if let Some(method) = req.method {
                    handshake_frame.set_method(method);
                }

                if let Some(path) = req.path {
                    handshake_frame.set_path(path);
                }

                if let Some(version) = req.version {
                    handshake_frame.set_version(version);
                }

                let mut headers = HashMap::new();
                for header in req.headers {
                    // Duplicate headers are concatenated as comma-separated string.
                    let key = header.name.to_string();
                    let val = String::from_utf8_lossy(header.value).into_owned();
                    let entry = headers.entry(key).or_insert_with(String::new);

                    if entry.is_empty() {
                        entry.push_str(&val);
                    } else {
                        entry.push(',');
                        entry.push_str(&val);
                    }
                }

                // Required Headers
                handshake_frame.set_host(headers.remove("Host"));
                handshake_frame.set_upgrade(headers.remove("Upgrade"));
                handshake_frame.set_conn(headers.remove("Connection"));
                handshake_frame.set_ws_key(headers.remove("Sec-WebSocket-Key"));
                handshake_frame.set_ws_version(headers.remove("Sec-WebSocket-Version"));

                // Optional headers
                handshake_frame.set_origin(headers.remove("Origin"));
                handshake_frame.set_protocol(headers.remove("Sec-WebSocket-Protocol"));
                handshake_frame.set_extensions(headers.remove("Sec-WebSocket-Extensions"));

                if !headers.is_empty() {
                    handshake_frame.set_others(headers);
                }

                if handshake_frame.validate() {
                    Ok(Some(handshake_frame))
                } else {
                    return Err(util::other("invalid handshake request"));
                }
            } else {
                return Ok(None);
            }
        } else {
            return Err(util::other("unable to parse client request"));
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

            if let Some(ref ext_resp) = self.ext_resp {
                if !ext_resp.is_empty() {
                    response.push_str(ext_resp);
                    response.push_str("\r\n");
                }
            }
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

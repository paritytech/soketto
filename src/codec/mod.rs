use bytes::BytesMut;
use crate::{codec::base::BaseCodec, frame::WebSocket, frame::handshake::Invalid, Nonce};
use std::{fmt, io};
use tokio_io::codec::{Decoder, Encoder};

pub mod http;
pub mod base;

/// Codec for use with the websocket protocol.
pub struct WebSocketCodec {
    /// The codec to use to decode the buffer into a `base::Frame`.
    base_codec: BaseCodec,
    /// The client-generated random nonce if any.
    nonce: Option<crate::Nonce>,
    /// The handshake indicator.  If this is false, the handshake is not complete.
    handshake_complete: bool
}

impl WebSocketCodec {
    pub fn server() -> Self {
        WebSocketCodec {
            nonce: None,
            base_codec: BaseCodec::new(),
            handshake_complete: false
        }
    }

    pub fn client(nonce: Nonce) -> Self {
        WebSocketCodec {
            nonce: Some(nonce),
            base_codec: BaseCodec::new(),
            handshake_complete: false
        }
    }
}

impl Decoder for WebSocketCodec {
    type Item = WebSocket;
    type Error = Error;

    fn decode(&mut self, buf: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if self.handshake_complete {
            if let Some(frame) = self.base_codec.decode(buf)? {
                return Ok(Some(WebSocket::Base(frame)))
            } else {
                return Ok(None)
            }
        }

        if let Some(nonce) = self.nonce.take() { // decode server response
            if let Some(http_resp) = http::ResponseHeaderCodec::new().decode(buf)? {
                let r = crate::frame::handshake::server::Response::new(nonce, http_resp)?;
                self.handshake_complete = true;
                Ok(Some(WebSocket::ServerResponse(r)))
            } else {
                Ok(None)
            }
        } else { // decode client request
            if let Some(http_req) = http::RequestHeaderCodec::new().decode(buf)? {
                let r = crate::frame::handshake::client::Request::new(http_req)?;
                Ok(Some(WebSocket::ClientRequest(r)))
            } else {
                Ok(None)
            }
        }
    }
}

impl Encoder for WebSocketCodec {
    type Item = WebSocket;
    type Error = Error;

    fn encode(&mut self, msg: Self::Item, buf: &mut BytesMut) -> Result<(), Self::Error> {
        match msg {
            WebSocket::ClientRequest(request) => {
                assert!(!self.handshake_complete);
                http::RequestHeaderCodec::new().encode(request.as_http(), buf)?;
                Ok(())
            }
            WebSocket::ServerResponse(response) => {
                assert!(!self.handshake_complete);
                http::ResponseHeaderCodec::new().encode(response.as_http(), buf)?;
                self.handshake_complete = true;
                Ok(())
            }
            WebSocket::Base(frame) => {
                assert!(self.handshake_complete);
                self.base_codec.encode(frame, buf)?;
                Ok(())
            }
        }
    }
}

// Error //////////////////////////////////////////////////////////////////////////////////////////

#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    Http(http::Error),
    Base(base::Error),
    Invalid(Invalid),

    #[doc(hidden)]
    __Nonexhaustive
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::Io(e) => write!(f, "i/o error: {}", e),
            Error::Http(e) => write!(f, "http error: {}", e),
            Error::Base(e) => write!(f, "base frame error: {}", e),
            Error::Invalid(i) => write!(f, "{}", i),
            Error::__Nonexhaustive => f.write_str("__Nonexhaustive")
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io(e) => Some(e),
            Error::Http(e) => Some(e),
            Error::Base(e) => Some(e),
            Error::Invalid(e) => Some(e),
            Error::__Nonexhaustive => None
        }
    }
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Error::Io(e)
    }
}

impl From<http::Error> for Error {
    fn from(e: http::Error) -> Self {
        Error::Http(e)
    }
}

impl From<base::Error> for Error {
    fn from(e: base::Error) -> Self {
        Error::Base(e)
    }
}

impl From<Invalid> for Error {
    fn from(e: Invalid) -> Self {
        Error::Invalid(e)
    }
}


#[cfg(test)]
mod test {
    use super::WebSocketCodec;
    use bytes::BytesMut;
    use crate::frame::WebSocket;
    use crate::frame::base::{Frame, Header, OpCode};
    use std::io::{self, Write};
    use tokio_io::codec::{Decoder, Encoder};

    const SHORT:  [u8; 7]   = [0x81, 0x81, 0x00, 0x00, 0x00, 0x01, 0x00];
    const MID:    [u8; 134] = [0x81, 0xFE, 0x00, 0x7E, 0x00, 0x00, 0x00, 0x01,
                               0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                               0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                               0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                               0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                               0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                               0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                               0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                               0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                               0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                               0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                               0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                               0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                               0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                               0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                               0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                               0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
    const CONT:   [u8; 7]   = [0x00, 0x81, 0x00, 0x00, 0x00, 0x01, 0x00];
    const TEXT:   [u8; 7]   = [0x81, 0x81, 0x00, 0x00, 0x00, 0x01, 0x00];
    const BINARY: [u8; 7]   = [0x82, 0x81, 0x00, 0x00, 0x00, 0x01, 0x00];
    const PING:   [u8; 7]   = [0x89, 0x81, 0x00, 0x00, 0x00, 0x01, 0x00];
    const PONG:   [u8; 7]   = [0x8A, 0x81, 0x00, 0x00, 0x00, 0x01, 0x00];

    fn decode_test(vec: Vec<u8>, opcode: OpCode) {
        let mut eb = BytesMut::with_capacity(256);
        eb.extend(vec);
        let mut fc = WebSocketCodec::server();
        fc.handshake_complete = true;

        match fc.decode(&mut eb) {
            Ok(Some(decoded)) => {
                if let WebSocket::Base(base) = decoded {
                    if base.header().opcode() == OpCode::Continue {
                        assert!(!base.header().is_fin());
                    } else {
                        assert!(base.header().is_fin());
                    }
                    assert!(!base.header().is_rsv1());
                    assert!(!base.header().is_rsv2());
                    assert!(!base.header().is_rsv3());
                    assert!(base.header().opcode() == opcode);
                    assert!(base.extension_data().is_empty());
                    assert!(!base.application_data().is_empty());
                } else {
                    assert!(false);
                }
            }
            Err(e) => {
                writeln!(io::stderr(), "{}", e).expect("Unable to write to stderr!");
                assert!(false);
            }
            _ => {
                assert!(false);
            }
        }
    }

    fn encode_test(cmp: Vec<u8>, opcode: OpCode, app_data: Vec<u8>) {
        let mut fc = WebSocketCodec::server();
        fc.handshake_complete = true;
        let mut header = Header::new(opcode);
        if opcode == OpCode::Continue {
            header.set_fin(false);
        }
        let mut base = Frame::from(header);
        base.set_application_data(app_data);

        let mut buf = BytesMut::with_capacity(1024);
        if let Ok(()) = <WebSocketCodec as Encoder>::encode(&mut fc, WebSocket::Base(base), &mut buf) {
            // There is no mask in encoded frames
            println!("b: {}, c: {}", buf.len(), cmp.len());
            assert!(buf.len() == (cmp.len() - 4));
            // TODO: Fix the comparision.  May have to just define separate encoded bufs.
            // for (a, b) in buf.iter().zip(cmp.iter()) {
            //     assert!(a == b);
            // }
        }
    }

    #[test]
    fn decode() {
        decode_test(SHORT.to_vec(), OpCode::Text);
        decode_test(MID.to_vec(), OpCode::Text);
        let mut long = Vec::with_capacity(65550);
        long.extend(&[0x81, 0xFF, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00,
                      0x00, 0x01]);
        long.extend([0; 65536].iter());
        decode_test(long, OpCode::Text);
        decode_test(CONT.to_vec(), OpCode::Continue);
        decode_test(TEXT.to_vec(), OpCode::Text);
        decode_test(BINARY.to_vec(), OpCode::Binary);
        decode_test(PING.to_vec(), OpCode::Ping);
        decode_test(PONG.to_vec(), OpCode::Pong);
    }

    #[test]
    fn encode() {
        encode_test(SHORT.to_vec(), OpCode::Text, vec![0]);
        encode_test(MID.to_vec(), OpCode::Text, vec![0; 126]);
        let mut long = Vec::with_capacity(65550);
        long.extend(&[0x81, 0xFF, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00,
                      0x00, 0x01]);
        long.extend([0; 65536].iter());
        encode_test(long.to_vec(), OpCode::Close, vec![0; 65536]);
        encode_test(CONT.to_vec(), OpCode::Continue, vec![0]);
        encode_test(TEXT.to_vec(), OpCode::Text, vec![0]);
        encode_test(BINARY.to_vec(), OpCode::Binary, vec![0]);
        encode_test(PING.to_vec(), OpCode::Ping, vec![0]);
        encode_test(PONG.to_vec(), OpCode::Pong, vec![0]);
    }
}

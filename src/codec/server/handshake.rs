//! Codec for dedoding/encoding websocket server handshake frames.
use base64::encode;
use frame::server::handshake::Frame;
use httparse::{EMPTY_HEADER, Request};
use sha1::Sha1;
use slog::Logger;
use std::collections::HashMap;
use std::io;
use tokio_core::io::{Codec, EasyBuf};
use util;

/// Defined in RFC6455 and used to generate the `Sec-WebSocket-Accept` header in the server
/// handshake response.
static KEY: &'static str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

#[derive(Default)]
/// Codec for decoding/encoding websocket server handshake frames.
pub struct FrameCodec {
    /// The request method (must be 'GET').
    method: String,
    /// The request path.
    path: String,
    /// The request version (must be 1).
    version: u8,
    /// Host header (Required)
    host: Option<String>,
    /// Upgrade header (Required)
    upgrade: Option<String>,
    /// Connection header (Required)
    conn: Option<String>,
    /// Sec-WebSocket-Key header (Required)
    ws_key: Option<String>,
    /// Sec-WebSocket-Version header (Required)
    ws_version: Option<String>,
    /// Origin header (Optional)
    origin: Option<String>,
    /// Sec-WebSocket-Protocol header (Optional)
    protocol: Option<String>,
    /// Sec-WebSocket-Extensions header (Optional)
    extensions: Option<String>,
    /// Any other remaining headers.
    others: HashMap<String, String>,
    /// Extension Negotiation Response.
    ext_resp: Option<String>,
    /// slog stdout `Logger`
    stdout: Option<Logger>,
    /// slog stderr `Logger`
    stderr: Option<Logger>,
}

impl FrameCodec {
    /// Add a stdout slog `Logger` to this protocol.
    pub fn stdout(&mut self, logger: Logger) -> &mut FrameCodec {
        let stdout = logger.new(o!("codec" => "handshake"));
        self.stdout = Some(stdout);
        self
    }

    /// Add a stderr slog `Logger` to this protocol.
    pub fn stderr(&mut self, logger: Logger) -> &mut FrameCodec {
        let stderr = logger.new(o!("codec" => "handshake"));
        self.stderr = Some(stderr);
        self
    }

    /// Set the extension negotiation response.
    pub fn set_ext_resp(&mut self, response: &str) -> &mut FrameCodec {
        self.ext_resp = Some(String::from(response));
        self
    }

    /// Validate the client handshake request.
    fn validate(&mut self) -> bool {
        if self.method != "GET" {
            return false;
        }

        if self.version != 1 {
            return false;
        }

        // TODO: Host Validation

        if let Some(ref val) = self.upgrade {
            if val.to_lowercase() != "websocket" {
                return false;
            }
        } else {
            return false;
        }

        if let Some(ref val) = self.conn {
            if val.to_lowercase() != "upgrade" {
                return false;
            }
        } else {
            return false;
        }

        if self.ws_key.is_none() {
            return false;
        }

        if let Some(ref val) = self.ws_version {
            if val != "13" {
                return false;
            }
        } else {
            return false;
        }

        true
    }

    /// Generate the `Sec-WebSocket-Accept` value.
    fn accept_val(&self, key: String) -> Result<String, io::Error> {
        if key.is_empty() {
            Err(util::other("invalid Sec-WebSocket-Key"))
        } else {
            let mut base = String::from(key);
            base.push_str(KEY);

            let mut m = Sha1::new();
            m.reset();
            m.update(base.as_bytes());

            Ok(encode(&m.digest().bytes()))
        }
    }
}

impl Codec for FrameCodec {
    type In = Frame;
    type Out = Frame;

    fn decode(&mut self, buf: &mut EasyBuf) -> Result<Option<Self::In>, io::Error> {
        let len = buf.len();
        if len == 0 {
            return Ok(None);
        }
        let drained = buf.drain_to(len);
        let req_bytes = drained.as_slice();
        let mut headers = [EMPTY_HEADER; 32];
        let mut req = Request::new(&mut headers);
        let mut handshake_frame: Frame = Default::default();

        if let Ok(res) = req.parse(req_bytes) {
            if res.is_complete() {
                if let Some(method) = req.method {
                    self.method = method.to_string();
                }

                if let Some(path) = req.path {
                    self.path = path.to_string();
                }

                if let Some(version) = req.version {
                    self.version = version;
                }

                let mut headers = HashMap::new();
                for header in req.headers {
                    // Duplicate headers are concatenated as comma-separated string.
                    let key = header.name.to_string();
                    let val = String::from_utf8_lossy(header.value).into_owned();
                    let mut entry = headers.entry(key).or_insert_with(String::new);

                    if entry.is_empty() {
                        entry.push_str(&val);
                    } else {
                        entry.push(',');
                        entry.push_str(&val);
                    }
                }

                // Required Headers
                self.host = headers.remove("Host");
                self.upgrade = headers.remove("Upgrade");
                self.conn = headers.remove("Connection");
                self.ws_key = headers.remove("Sec-WebSocket-Key");
                self.ws_version = headers.remove("Sec-WebSocket-Version");

                // Optional headers
                self.origin = headers.remove("Origin");
                self.protocol = headers.remove("Sec-WebSocket-Protocol");
                self.extensions = headers.remove("Sec-WebSocket-Extensions");

                if !headers.is_empty() {
                    self.others = headers;
                }

                if self.validate() {
                    handshake_frame.set_extensions(self.extensions.clone());
                    handshake_frame.set_key(self.ws_key.clone());
                    Ok(Some(handshake_frame))
                } else {
                    return Err(util::other("invalid handshake request"));
                }
            } else {
                return Err(util::other("partial client request received"));
            }
        } else {
            return Err(util::other("unable to parse client request"));
        }
    }

    fn encode(&mut self, msg: Self::Out, buf: &mut Vec<u8>) -> io::Result<()> {
        let mut response = String::from("HTTP/1.1 101 Switching Protocols\r\n");
        response.push_str("Upgrade: websocket\r\n");
        response.push_str("Connection: upgrade\r\n");
        response.push_str(&format!("Sec-WebSocket-Accept: {}\r\n", self.accept_val(msg.key())?));

        // TODO: Add support for 400 response, subprotocols.
        if let Some(ref ext_resp) = self.ext_resp {
            if !ext_resp.is_empty() {
                response.push_str(ext_resp);
                response.push_str("\r\n");
            }
        }

        response.push_str("\r\n");

        try_trace!(self.stdout, "handshake response\n{}", response);
        buf.extend(response.as_bytes());
        Ok(())
    }
}


#[cfg(test)]
mod test {
    use super::FrameCodec;

    #[test]
    pub fn accept() {
        let hf: FrameCodec = Default::default();
        if let Ok(res) = hf.accept_val("dGhlIHNhbXBsZSBub25jZQ==".to_string()) {
            assert!(res == "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=");
        } else {
            assert!(false);
        }
    }
}

//! websocket handshake frame
use base64::encode;
use httparse::{EMPTY_HEADER, Request};
use sha1::Sha1;
use std::collections::HashMap;
use std::fmt;
use std::io;
use tokio_core::io::EasyBuf;
use util;

/// Defined in RFC6455 and used to generate the `Sec-WebSocket-Accept` header in the server
/// handshake response.
static KEY: &'static str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

#[derive(Debug, Clone)]
/// The `Frame` struct.
pub struct Frame {
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
}

// TODO: Convert to return result with reason code.
impl Frame {
    /// Get the `ws_key`.
    pub fn ws_key(&self) -> String {
        let mut res = String::new();

        if let Some(ref key) = self.ws_key {
            res.push_str(key);
        }
        res
    }

    /// Get the `extensions`
    pub fn extensions(&self) -> String {
        let mut res = String::new();

        if let Some(ref extensions) = self.extensions {
            res.push_str(extensions);
        }
        res
    }

    /// Validate the client handshake request.
    fn validate(&mut self, handshake: &Frame) -> bool {
        if handshake.method != "GET" {
            return false;
        }

        if handshake.version != 1 {
            return false;
        }

        // TODO: Host Validation

        if let Some(ref val) = handshake.upgrade {
            if val.to_lowercase() != "websocket" {
                return false;
            }
        } else {
            return false;
        }

        if let Some(ref val) = handshake.conn {
            if val.to_lowercase() != "upgrade" {
                return false;
            }
        } else {
            return false;
        }

        if handshake.ws_key.is_none() {
            return false;
        }

        if let Some(ref val) = handshake.ws_version {
            if val != "13" {
                return false;
            }
        } else {
            return false;
        }

        true
    }

    /// Decode and `EasyBuf` into a `Frame`.
    pub fn decode(&mut self, buf: &mut EasyBuf) -> Result<Option<Frame>, io::Error> {
        let len = buf.len();
        let drained = buf.drain_to(len);
        let req_bytes = drained.as_slice();
        let mut headers = [EMPTY_HEADER; 32];
        let mut req = Request::new(&mut headers);
        let mut handshake_frame: Frame = Default::default();

        if let Ok(res) = req.parse(req_bytes) {
            if res.is_complete() {
                if let Some(method) = req.method {
                    handshake_frame.method = method.to_string();
                }

                if let Some(path) = req.path {
                    handshake_frame.path = path.to_string();
                }

                if let Some(version) = req.version {
                    handshake_frame.version = version;
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
                handshake_frame.host = headers.remove("Host");
                handshake_frame.upgrade = headers.remove("Upgrade");
                handshake_frame.conn = headers.remove("Connection");
                handshake_frame.ws_key = headers.remove("Sec-WebSocket-Key");
                handshake_frame.ws_version = headers.remove("Sec-WebSocket-Version");

                // Optional headers
                handshake_frame.origin = headers.remove("Origin");
                handshake_frame.protocol = headers.remove("Sec-WebSocket-Protocol");
                handshake_frame.extensions = headers.remove("Sec-WebSocket-Extensions");

                if !headers.is_empty() {
                    handshake_frame.others = headers;
                }

                if self.validate(&handshake_frame) {
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

    /// Generate the `Sec-WebSocket-Accept` value.
    fn accept_val(&self) -> Result<String, io::Error> {
        if let Some(ref ws_key) = self.ws_key {
            let mut base = ws_key.clone();
            base.push_str(KEY);

            let mut m = Sha1::new();
            m.reset();
            m.update(base.as_bytes());

            Ok(encode(&m.digest().bytes()))
        } else {
            Err(util::other("invalid handshake frame"))
        }
    }

    #[cfg(feature = "pmdeflate")]
    /// Add the `Sec-WebSocket-Extensions: permessage-deflate` header to the response.
    fn ext_header(&self, buf: &mut String) {
        if let Some(ref extensions) = self.extensions {
            if extensions.contains("permessage-deflate") {
                buf.push_str("Sec-WebSocket-Extensions: permessage-deflate; \
                server_no_context_takeover\r\n");
            }
        }
    }

    #[cfg(not(feature = "pmdeflate"))]
    /// Does nothing when `pmdeflate` feature is disabled.
    fn ext_header(&self, _buf: &mut String) {}

    /// Convert a `Frame` into a byte buffer.
    pub fn to_byte_buf(&self, buf: &mut Vec<u8>) -> Result<(), io::Error> {
        let mut response = String::from("HTTP/1.1 101 Switching Protocols\r\n");
        response.push_str("Upgrade: websocket\r\n");
        response.push_str("Connection: upgrade\r\n");
        response.push_str(&format!("Sec-WebSocket-Accept: {}\r\n", try!(self.accept_val())));

        // TODO: Add support for 400 response, subprotocols.
        self.ext_header(&mut response);

        response.push_str("\r\n");


        buf.extend(response.as_bytes());
        Ok(())
    }
}

impl Default for Frame {
    fn default() -> Frame {
        Frame {
            method: String::new(),
            path: String::new(),
            version: 0,
            host: None,
            upgrade: None,
            conn: None,
            ws_key: None,
            ws_version: None,
            origin: None,
            protocol: None,
            extensions: None,
            others: HashMap::new(),
        }
    }
}

impl fmt::Display for Frame {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        try!(writeln!(f, "Frame {{"));
        try!(writeln!(f, "\tmethod: {}", self.method));
        try!(writeln!(f, "\tpath: {}", self.path));
        try!(writeln!(f, "\tversion: {}", self.version));
        if let Some(ref host) = self.host {
            try!(writeln!(f, "\thost: {}", host));
        }
        if let Some(ref upgrade) = self.upgrade {
            try!(writeln!(f, "\tupgrade: {}", upgrade));
        }
        if let Some(ref conn) = self.conn {
            try!(writeln!(f, "\tconn: {}", conn));
        }
        if let Some(ref ws_key) = self.ws_key {
            try!(writeln!(f, "\tws_key: {}", ws_key));
        }
        if let Some(ref ws_version) = self.ws_version {
            try!(writeln!(f, "\tws_version: {}", ws_version));
        }
        if let Some(ref origin) = self.origin {
            try!(writeln!(f, "\torigin: {}", origin));
        }
        if let Some(ref protocol) = self.protocol {
            try!(writeln!(f, "\tprotocol: {}", protocol));
        }
        if let Some(ref extensions) = self.extensions {
            try!(writeln!(f, "\textensions: {}", extensions));
        }
        if !self.others.is_empty() {
            for (k, v) in &self.others {
                try!(writeln!(f, "\t{}: {}", k, v));
            }
        }
        write!(f, "}}")
    }
}

#[cfg(test)]
mod test {
    use super::Frame;

    #[test]
    pub fn accept() {
        let mut hf: Frame = Default::default();
        hf.ws_key = Some("dGhlIHNhbXBsZSBub25jZQ==".to_string());
        if let Ok(res) = hf.accept_val() {
            assert!(res == "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=");
        } else {
            assert!(false);
        }
    }
}

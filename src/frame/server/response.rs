//! Server response frame to a client request
use base64::encode;
use sha1::Sha1;
use std::fmt;
use std::io;
use util;

/// Defined in RFC6455 and used to generate the `Sec-WebSocket-Accept` header in the server
/// handshake response.
static KEY: &'static str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

/// Data needed to construct a server response to a client request.
#[derive(Clone, Debug, Default)]
pub struct Frame {
    /// Extensions header from request.
    extensions: String,
    /// `Sec-WebSocket-Key` from request.
    ws_key: String,
}

impl Frame {
    /// Get the `ws_key` value.
    pub fn ws_key(&self) -> &str {
        &self.ws_key
    }

    /// Set the `ws_key` value.
    pub fn set_ws_key(&mut self, ws_key: String) -> &mut Frame {
        self.ws_key = ws_key;
        self
    }

    /// Get the `extensions` value.
    pub fn extensions(&self) -> &str {
        &self.extensions
    }

    /// Set the `ws_key` value.
    pub fn set_extensions(&mut self, extensions: String) -> &mut Frame {
        self.extensions = extensions;
        self
    }

    /// Generate the `Sec-WebSocket-Accept` value.
    pub fn accept_val(&self) -> io::Result<String> {
        if self.ws_key.is_empty() {
            Err(util::other("invalid Sec-WebSocket-Key"))
        } else {
            let mut base = self.ws_key.clone();
            base.push_str(KEY);

            let mut m = Sha1::new();
            m.reset();
            m.update(base.as_bytes());

            Ok(encode(&m.digest().bytes()))
        }
    }
}

impl fmt::Display for Frame {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "server side handshake response frame")
    }
}

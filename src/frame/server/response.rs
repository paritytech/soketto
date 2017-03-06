//! Server response frame to a client request
use base64::encode;
use sha1::Sha1;
use std::collections::HashMap;
use std::fmt;
use std::io;
use util;

/// Defined in RFC6455 and used to generate the `Sec-WebSocket-Accept` header in the server
/// handshake response.
static KEY: &'static str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

/// Data needed to construct a server response to a client request.
#[derive(Clone, Debug, Default)]
pub struct Frame {
    /// The response `code`
    code: u16,
    /// The response `reason`
    reason: String,
    /// Upgrade header (Required for 101 response)
    upgrade: Option<String>,
    /// Connection header (Required for 101 respones)
    conn: Option<String>,
    /// `Sec-WebSocket-Key` from request.
    ws_key: String,
    /// Sec-WebSocket-Protocol header (Optional)
    protocol: Option<String>,
    /// Sec-WebSocket-Extensions header (Optional)
    extensions: Option<String>,
    /// Any other remaining headers.
    others: HashMap<String, String>,
}

impl Frame {
    /// Get the `code` value.
    pub fn code(&self) -> u16 {
        self.code
    }

    /// Set the `code` value {
    pub fn set_code(&mut self, code: u16) -> &mut Frame {
        self.code = code;
        self
    }

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
    pub fn extensions(&self) -> String {
        let mut res = String::new();

        if let Some(ref extensions) = self.extensions {
            res.push_str(extensions);
        }
        res
    }

    /// Set the `extensions` value.
    pub fn set_extensions(&mut self, extensions: Option<String>) -> &mut Frame {
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

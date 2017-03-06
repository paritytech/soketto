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
#[derive(Clone, Debug)]
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

    /// Get the `reason` value.
    pub fn reason(&self) -> &str {
        &self.reason
    }

    /// Set the `reason` value {
    pub fn set_reason(&mut self, reason: &str) -> &mut Frame {
        self.reason = reason.to_string();
        self
    }

    /// Get the `upgrade` value.
    pub fn upgrade(&self) -> String {
        let mut res = String::new();

        if let Some(ref upgrade) = self.upgrade {
            res.push_str(upgrade);
        }
        res
    }

    /// Set the `upgrade` value.
    pub fn set_upgrade(&mut self, upgrade: Option<String>) -> &mut Frame {
        self.upgrade = upgrade;
        self
    }

    /// Get the `conn` value.
    pub fn conn(&self) -> String {
        let mut res = String::new();

        if let Some(ref conn) = self.conn {
            res.push_str(conn);
        }
        res
    }

    /// Set the `conn` value.
    pub fn set_conn(&mut self, conn: Option<String>) -> &mut Frame {
        self.conn = conn;
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

    /// Get the `protocol` value.
    pub fn protocol(&self) -> String {
        let mut res = String::new();

        if let Some(ref protocol) = self.protocol {
            res.push_str(protocol);
        }
        res
    }

    /// Set the `protocol` value.
    pub fn set_protocol(&mut self, protocol: Option<String>) -> &mut Frame {
        self.protocol = protocol;
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

    /// Get the `others` value.
    pub fn others(&self) -> &HashMap<String, String> {
        &self.others
    }

    /// Set the `others` value.
    pub fn set_others(&mut self, others: HashMap<String, String>) -> &mut Frame {
        self.others = others;
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

impl Default for Frame {
    fn default() -> Frame {
        Frame {
            code: 101,
            reason: String::from("Switching Protocols"),
            upgrade: None,
            conn: None,
            ws_key: String::new(),
            protocol: None,
            extensions: None,
            others: HashMap::new(),
        }
    }
}

impl fmt::Display for Frame {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "Frame {{")?;
        writeln!(f, "\tcode: {}", self.code)?;
        writeln!(f, "\treason: {}", self.reason)?;
        writeln!(f, "\tws_key: {}", self.ws_key)?;

        if let Some(ref upgrade) = self.upgrade {
            writeln!(f, "\tupgrade: {}", upgrade)?;
        }

        if let Some(ref conn) = self.conn {
            writeln!(f, "\tconn: {}", conn)?;
        }

        if let Some(ref protocol) = self.protocol {
            writeln!(f, "\tprotocol: {}", protocol)?;
        }

        if let Some(ref extensions) = self.extensions {
            writeln!(f, "\textensions: {}", extensions)?;
        }

        for (k, v) in self.others.iter() {
            writeln!(f, "\t{}: {}", *k, *v)?;
        }

        writeln!(f, "}}")
    }
}

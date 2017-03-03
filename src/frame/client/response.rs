//! A server handshake response frame.
use std::collections::HashMap;
use std::fmt;

/// A server handshake response to a client handshake request.
#[derive(Default, Debug, Clone)]
pub struct Frame {
    /// The response `version`
    version: u8,
    /// The response `code`
    code: u16,
    /// The response `reason`
    reason: String,
    /// Upgrade header (Required)
    upgrade: Option<String>,
    /// Connection header (Required)
    conn: Option<String>,
    /// Sec-WebSocket-Accept header (Required)
    ws_accept: Option<String>,
    /// Sec-WebSocket-Protocol header (Optional)
    protocol: Option<String>,
    /// Sec-WebSocket-Extensions header (Optional)
    extensions: Option<String>,
    /// Any other remaining headers.
    others: HashMap<String, String>,
}

impl Frame {
    /// Get the `version` value.
    pub fn version(&self) -> u8 {
        self.version
    }

    /// Set the `version` value.
    pub fn set_version(&mut self, version: u8) -> &mut Frame {
        self.version = version;
        self
    }

    /// Get the `code` value.
    pub fn code(&self) -> u16 {
        self.code
    }

    /// Set the `code` value.
    pub fn set_code(&mut self, code: u16) -> &mut Frame {
        self.code = code;
        self
    }

    /// Get the `reason` value.
    pub fn reason(&self) -> &str {
        &self.reason
    }

    /// Set the `reason` value.
    pub fn set_reason(&mut self, reason: &str) -> &mut Frame {
        self.reason = reason.into();
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

    /// Get the `ws_accept` value.
    pub fn ws_accept(&self) -> String {
        let mut res = String::new();

        if let Some(ref ws_accept) = self.ws_accept {
            res.push_str(ws_accept);
        }
        res
    }

    /// Set the `ws_accept` value.
    pub fn set_ws_accept(&mut self, ws_accept: Option<String>) -> &mut Frame {
        self.ws_accept = ws_accept;
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

    /// Set the `extensisons` value.
    pub fn set_extensions(&mut self, extensions: Option<String>) -> &mut Frame {
        self.extensions = extensions;
        self
    }

    /// Get the `others` value.
    pub fn others(&self) -> HashMap<String, String> {
        self.others.clone()
    }

    /// Set the `others` value.
    pub fn set_others(&mut self, others: HashMap<String, String>) -> &mut Frame {
        self.others = others;
        self
    }

    /// Validate this server response frame.
    pub fn validate(&self) -> bool {
        if self.version != 1 {
            return false;
        }

        if self.reason.to_lowercase() != "switching protocols" {
            return false;
        }

        if self.code != 101 {
            return false;
        }

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

        if self.ws_accept.is_none() {
            return false;
        }

        true
    }
}

impl fmt::Display for Frame {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "Frame {{")?;
        writeln!(f, "\tversion: {}", self.version)?;
        writeln!(f, "\tcode: {}", self.code)?;
        writeln!(f, "\treason: {}", self.reason)?;
        if let Some(ref upgrade) = self.upgrade {
            writeln!(f, "\tupgrade: {}", upgrade)?;
        } else {
            writeln!(f, "\tupgrade: None")?;
        }

        if let Some(ref conn) = self.conn {
            writeln!(f, "\tconn: {}", conn)?;
        } else {
            writeln!(f, "\tconn: None")?;
        }

        if let Some(ref ws_accept) = self.ws_accept {
            writeln!(f, "\tws_accept: {}", ws_accept)?;
        } else {
            writeln!(f, "\tws_accept: None")?;
        }

        writeln!(f, "}}")
    }
}

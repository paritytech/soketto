//! Client handshake frame received by server.
use std::collections::HashMap;
use std::fmt;

/// Client handshake request data received by server.
#[derive(Clone, Debug, Default)]
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
    /// Other headers (Optional)
    others: HashMap<String, String>,
}

impl Frame {
    /// Get the `method` value.
    pub fn method(&self) -> &str {
        &self.method
    }

    /// Set the `method` value.
    pub fn set_method(&mut self, method: &str) -> &mut Frame {
        self.method = method.into();
        self
    }

    /// Get the `path` value.
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Set the `path` value.
    pub fn set_path(&mut self, path: &str) -> &mut Frame {
        self.path = path.into();
        self
    }

    /// Get the `version` value.
    pub fn version(&self) -> u8 {
        self.version
    }

    /// Set the `version` value.
    pub fn set_version(&mut self, version: u8) -> &mut Frame {
        self.version = version;
        self
    }

    /// Get the `host` value.
    pub fn host(&self) -> String {
        let mut res = String::new();

        if let Some(ref host) = self.host {
            res.push_str(host);
        }
        res
    }

    /// Set the `host` value.
    pub fn set_host(&mut self, host: Option<String>) -> &mut Frame {
        self.host = host;
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
    pub fn ws_key(&self) -> String {
        let mut res = String::new();

        if let Some(ref ws_key) = self.ws_key {
            res.push_str(ws_key);
        }
        res
    }

    /// Set the `ws_key` value.
    pub fn set_ws_key(&mut self, ws_key: Option<String>) -> &mut Frame {
        self.ws_key = ws_key;
        self
    }

    /// Get the `ws_version` value.
    pub fn ws_version(&self) -> String {
        let mut res = String::new();

        if let Some(ref ws_version) = self.ws_version {
            res.push_str(ws_version);
        }
        res
    }

    /// Set the `ws_version` value.
    pub fn set_ws_version(&mut self, ws_version: Option<String>) -> &mut Frame {
        self.ws_version = ws_version;
        self
    }

    /// Get the `origin` value.
    pub fn ws_origin(&self) -> String {
        let mut res = String::new();

        if let Some(ref origin) = self.origin {
            res.push_str(origin);
        }
        res
    }

    /// Set the `origin` value.
    pub fn set_origin(&mut self, origin: Option<String>) -> &mut Frame {
        self.origin = origin;
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

    /// Validate the client handshake request.
    pub fn validate(&mut self) -> bool {
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
}

impl fmt::Display for Frame {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "server side handshake request frame")
    }
}

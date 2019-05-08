//! websocket handshake client-side frame
use std::{collections::HashMap, fmt};

/// A websocket handshake client-side frame.
#[derive(Clone, Debug, Default)]
pub struct Frame {
    /// The URL path.
    path: String,
    /// The URL query string.
    query: String,
    /// The `user_agent` header value.
    user_agent: String,
    /// The `host` header value.
    host: String,
    /// The `origin` header value.
    origin: String,
    /// The `sec_websocket_key` header value.
    sec_websocket_key: String,
    /// Other headers.
    others: HashMap<String, String>,
}

impl Frame {
    /// Get the `path` value.
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Set the `path` value.
    pub fn set_path(&mut self, path: String) -> &mut Frame {
        self.path = path;
        self
    }

    /// Get the `query` value.
    pub fn query(&self) -> &str {
        &self.query
    }

    /// Set the `query` value.
    pub fn set_query(&mut self, query: String) -> &mut Frame {
        self.query = query;
        self
    }

    /// Get the `user_agent` value.
    pub fn user_agent(&self) -> &str {
        &self.user_agent
    }

    /// Set the `user_agent` value.
    pub fn set_user_agent(&mut self, user_agent: String) -> &mut Frame {
        self.user_agent = user_agent;
        self
    }

    /// Get the `host` value.
    pub fn host(&self) -> &str {
        &self.host
    }

    /// Set the `host` value.
    pub fn set_host(&mut self, host: String) -> &mut Frame {
        self.host = host;
        self
    }

    /// Get the `origin` value.
    pub fn origin(&self) -> &str {
        &self.origin
    }

    /// Set the `origin` value.
    pub fn set_origin(&mut self, origin: String) -> &mut Frame {
        self.origin = origin;
        self
    }

    /// Get the `sec_websocket_key` value.
    pub fn sec_websocket_key(&self) -> &str {
        &self.sec_websocket_key
    }

    /// Set the `sec_websocket_key` value.
    pub fn set_sec_websocket_key(&mut self, sec_websocket_key: String) -> &mut Frame {
        self.sec_websocket_key = sec_websocket_key;
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
}

impl fmt::Display for Frame {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Client Handshake Frame")
    }
}

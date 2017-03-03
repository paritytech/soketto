//! websocket handshake client-side frame
use std::fmt;

#[derive(Clone, Debug, Default)]
/// A websocket handshake client-side frame.
pub struct Frame {
    /// The `user_agent` header value.
    user_agent: String,
    /// The `host` header value.
    host: String,
    /// The `sec_websocket_key` header value.
    sec_websocket_key: String,
}

impl Frame {
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

    /// Get the `sec_websocket_key` value.
    pub fn sec_websocket_key(&self) -> &str {
        &self.sec_websocket_key
    }

    /// Set the `sec_websocket_key` value.
    pub fn set_sec_websocket_key(&mut self, sec_websocket_key: String) -> &mut Frame {
        self.sec_websocket_key = sec_websocket_key;
        self
    }
}

impl fmt::Display for Frame {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Client Handshake Frame")
    }
}

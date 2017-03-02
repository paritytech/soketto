//! websocket handshake client-side frame
use base64::encode;
use byteorder::{BigEndian, WriteBytesExt};
use rand::{self, Rng};
use std::fmt;
use std::io;

#[derive(Clone, Debug)]
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
    /// Create a new client handshake frame.
    pub fn new() -> io::Result<Frame> {
        let mut rng = rand::thread_rng();
        let mut nonce_vec = Vec::with_capacity(2);
        let nonce = rng.gen::<u16>();
        nonce_vec.write_u16::<BigEndian>(nonce)?;

        let sec_websocket_key = encode(&nonce_vec);
        Ok(Frame {
               user_agent: String::new(),
               host: String::new(),
               sec_websocket_key: sec_websocket_key,
           })
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

    /// Get the `sec_websocket_key` value.
    pub fn sec_websocket_key(&self) -> &str {
        &self.sec_websocket_key
    }
}

impl fmt::Display for Frame {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Client Handshake Frame")
    }
}

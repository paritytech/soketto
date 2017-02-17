//! websocket handshake frame
use std::fmt;

#[derive(Clone, Debug, Default)]
/// A websocket handshake frame.
pub struct Frame {
    /// The enabled extensions.
    extensions: Option<String>,
    /// Handshake Key
    key: Option<String>,
}

impl Frame {
    /// Get the `extensions`
    pub fn extensions(&self) -> String {
        let mut res = String::new();

        if let Some(ref extensions) = self.extensions {
            res.push_str(extensions);
        }
        res
    }

    /// Set the `extensions`
    pub fn set_extensions(&mut self, extensions: Option<String>) -> &mut Frame {
        self.extensions = extensions;
        self
    }

    /// Get the `key`
    pub fn key(&self) -> String {
        let mut res = String::new();

        if let Some(ref key) = self.key {
            res.push_str(key);
        }
        res
    }

    /// Set the `key`
    pub fn set_key(&mut self, key: Option<String>) -> &mut Frame {
        self.key = key;
        self
    }
}

impl fmt::Display for Frame {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if let Some(ref ext) = self.extensions {
            write!(f, "{}", ext)
        } else {
            write!(f, "No enabled extensions")
        }
    }
}

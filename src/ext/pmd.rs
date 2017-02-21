//! Per-message Deflate Extension
use super::{FromHeader, IntoResponse, PerMessage};

/// The per-message deflate state.
pub struct Deflate {
    /// The max size of the sliding window. If the other endpoint selects a smaller size, that size
    /// will be used instead. This must be an integer between 8 and 15 inclusive.
    /// Default: 15
    pub max_window_bits: u8,
    /// Indicates whether to ask the other endpoint to reset the sliding window for each message.
    /// Default: false
    pub request_no_context_takeover: bool,
    /// Indicates whether this endpoint will agree to reset the sliding window for each message it
    /// compresses. If this endpoint won't agree to reset the sliding window, then the handshake
    /// will fail if this endpoint is a client and the server requests no context takeover.
    /// Default: true
    pub accept_no_context_takeover: bool,
}

impl Default for Deflate {
    fn default() -> Deflate {
        Deflate {
            max_window_bits: 15,
            request_no_context_takeover: false,
            accept_no_context_takeover: true,
        }
    }
}

impl FromHeader for Deflate {
    fn build(&self, request: &str) -> Self {
        stdout_trace!("extension" => "pmd"; "Building Deflate from {}", request);
        Default::default()
    }
}

impl IntoResponse for Deflate {
    fn response(&self) -> String {
        String::new()
    }
}

impl PerMessage for Deflate {
    fn decode(&self, _message: Vec<u8>) -> Vec<u8> {
        Vec::new()
    }

    fn encode(&self, _message: Vec<u8>) -> Vec<u8> {
        Vec::new()
    }
}

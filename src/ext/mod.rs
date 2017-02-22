//! Per-message Compression Extensions (pmce)
//!
//! Currently, only per-message deflate is supported, if enabled.

/// Extensions are built from the `Sec-WebSocket-Extensions` headers.  Build your extension based on
/// that header.
pub trait FromHeader {
    /// Initialize your extension from the given `Sec-WebSocket-Extensions` header string.
    fn init(&mut self, header: &str);
}

/// Server-side extensions should implement this to
pub trait IntoResponse {
    /// Generate the `Sec-WebSocket-Extensions` portion of your response.
    fn response(&self) -> String;
}

/// A per-message extension.
pub trait PerMessage: FromHeader + IntoResponse + Send {
    /// Transform the given application data/extension data bytes as necessary.
    fn decode(&self, message: Vec<u8>) -> Vec<u8>;
    /// Transform the given bytes into application/extension data bytes as necessary.
    fn encode(&self, message: Vec<u8>) -> Vec<u8>;
}

/// A per-frame extension.
pub trait PerFrame: FromHeader + IntoResponse + Send {
    /// Transform the given application data/extension data bytes as necessary.
    fn decode(&self, message: Vec<u8>) -> Vec<u8>;
    /// Transform the given bytes into application/extension data bytes as necessary.
    fn encode(&self, message: Vec<u8>) -> Vec<u8>;
}

//! Per-message Compression Extensions (pmce)
//!
//! Currently, only per-message deflate is supported, if enabled.
#[cfg(feature = "pmd")]
pub mod pmd;

/// Extensions are built from the `Sec-WebSocket-Extensions` headers.  Build your extension based on
/// that header.
pub trait FromHeader {
    /// Build your extension from the given `Sec-WebSocket-Extensions` header string.
    fn build(&self, header: &str) -> Self;
}

/// Server-side extensions should implement this to
pub trait IntoResponse {
    /// Generate the `Sec-WebSocket-Extensions` portion of your response.
    fn response(&self) -> String;
}

/// A per-message extension.
pub trait PerMessage: IntoResponse {
    /// Transform the given application data/extension data bytes as necessary.
    fn decode(&self, message: Vec<u8>) -> Vec<u8>;
    /// Transform the given bytes into application/extension data bytes as necessary.
    fn encode(&self, message: Vec<u8>) -> Vec<u8>;
}

/// A per-frame extension.
pub trait PerFrame: IntoResponse {
    /// Transform the given application data/extension data bytes as necessary.
    fn decode(&self, message: Vec<u8>) -> Vec<u8>;
    /// Transform the given bytes into application/extension data bytes as necessary.
    fn encode(&self, message: Vec<u8>) -> Vec<u8>;
}

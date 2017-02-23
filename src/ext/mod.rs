//! Per-message Compression Extensions (pmce)
//!
//! Currently, only per-message deflate is supported, if enabled.
use frame::base;
use std::collections::HashMap;
use std::io;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

/// Thread safe ref counted storage for user supplied per-message extensions.
pub type PerMessageExtensions = Arc<Mutex<HashMap<Uuid, Vec<Box<PerMessage>>>>>;
/// Thread safe ref counted storage for user supplied per-frameextensions.
pub type PerFrameExtensions = Arc<Mutex<HashMap<Uuid, Vec<Box<PerFrame>>>>>;

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
    /// Reserve `rsvX` bits for use by your extension.  Valid values are 0 - 8 (no rsv bits
    /// reserved, up to all 3).  If your bits are already reserved by an extension earlier in the
    /// chain, return an io::Error.
    fn reserve_rsv(&self, reserved_rsv: u8) -> Result<u8, io::Error>;
    /// If your extension uses the extension_data area of a websocket frame, this function should
    /// return true.
    fn uses_extension_data(&self) -> bool;
    /// Transform the given application data/extension data bytes as necessary.
    fn decode(&self, message: &mut base::Frame) -> Result<(), io::Error>;
    /// Transform the given bytes into application/extension data bytes as necessary.
    fn encode(&self, message: &mut base::Frame) -> Result<(), io::Error>;
}

/// A per-frame extension.
pub trait PerFrame: FromHeader + IntoResponse + Send {
    /// Reserve `rsvX` bits for use by your extension.  Valid values are 0 - 16 (no rsv bits
    /// reserved, up to all 4).  If your bits are already reserved by an extension earlier in the
    /// chain, return an io::Error.
    fn reserve_rsv(&self, reserved_rsv: u8) -> Result<u8, io::Error>;
    /// If your extension uses the extension_data area of a websocket frame, this function should
    /// return true.
    fn uses_extension_data(&self) -> bool;
    /// Transform the given application data/extension data bytes as necessary.
    fn decode(&self, message: &mut base::Frame) -> Result<(), io::Error>;
    /// Transform the given bytes into application/extension data bytes as necessary.
    fn encode(&self, message: &mut base::Frame) -> Result<(), io::Error>;
}

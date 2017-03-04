//! Extension Trait API
use frame::base;
use std::collections::HashMap;
use std::io;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

/// Thread safe ref counted storage for user supplied per-message extensions.
pub type PerMessageExtensions = Arc<Mutex<HashMap<Uuid, Vec<Box<PerMessage>>>>>;
/// Thread safe ref counted storage for user supplied per-frameextensions.
pub type PerFrameExtensions = Arc<Mutex<HashMap<Uuid, Vec<Box<PerFrame>>>>>;

/// Extensions are configured from the `Sec-WebSocket-Extensions` headers.  Configure your
/// extension based on that header.
pub trait Header {
    /// Initialize your extension from the given `Sec-WebSocket-Extensions` header
    /// string.  If your extension parameters exist, but are invalid, you should return an error
    /// here.  If they don't exist you should mark yourself disabled (see enabled below), and
    /// return Ok.  For a server-side extension, this will be called first, and should be used to
    /// generate the value that will be returned by `into_header`.   For a client-side extension,
    /// this will be called after the server response is received, and should be use to
    /// re-configure your extension based on the value.
    fn from_header(&mut self, header: &str) -> io::Result<()>;
    /// This should return your extensions `Sec-WebSocket-Extensions` header to be used in a
    /// request or response.   For a server-side extension, this will be called after `from_header`
    /// has been called and the extension is configures.   This result should represent your
    /// negotiation response.   For a client-side extension, this will be called to ge the header
    /// for an extension negotiation request.
    fn into_header(&mut self) -> io::Result<Option<String>>;
}

/// A per-message extension.
pub trait PerMessage: Header + Send {
    /// Is this extension enabled?  This should return true if you are able to support the given
    /// `Sec-WebSocket-Extensions` header parameters.  It should return false otherwise.
    fn enabled(&self) -> bool;
    /// Reserve `rsvX` bits for use by your extension.  Valid values are 0 - 8 (no rsv bits
    /// reserved, up to all 3).  If your bits are already reserved by an extension earlier in the
    /// chain, return an io::Error.
    fn reserve_rsv(&self, reserved_rsv: u8) -> Result<u8, io::Error>;
    /// Transform the given application data/extension data bytes as necessary.
    fn decode(&self, message: &mut base::Frame) -> Result<(), io::Error>;
    /// Transform the given bytes into application/extension data bytes as necessary.
    fn encode(&self, message: &mut base::Frame) -> Result<(), io::Error>;
}

/// A per-frame extension.
pub trait PerFrame: Header + Send {
    /// Is this extension enabled?  This should return true if you are able to support the given
    /// `Sec-WebSocket-Extensions` header parameters.  It should return false otherwise.
    fn enabled(&self) -> bool;
    /// Reserve `rsvX` bits for use by your extension.  Valid values are 0 - 16 (no rsv bits
    /// reserved, up to all 4).  If your bits are already reserved by an extension earlier in the
    /// chain, return an io::Error.
    fn reserve_rsv(&self, reserved_rsv: u8) -> Result<u8, io::Error>;
    /// Transform the given application data/extension data bytes as necessary.
    fn decode(&self, message: &mut base::Frame) -> Result<(), io::Error>;
    /// Transform the given bytes into application/extension data bytes as necessary.
    fn encode(&self, message: &mut base::Frame) -> Result<(), io::Error>;
}

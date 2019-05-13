pub mod client;
pub mod server;

use std::fmt;

/// Defined in RFC6455 and used to generate the `Sec-WebSocket-Accept` header
/// in the server handshake response.
pub(crate) const KEY: &[u8] = b"258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

#[derive(Debug)]
pub struct Invalid(String);

impl Invalid {
    pub fn new<S: Into<String>>(s: S) -> Self {
        Invalid(s.into())
    }
}

impl fmt::Display for Invalid {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "invalid: {}", self.0)
    }
}

impl std::error::Error for Invalid {}

pub(crate) fn expect_header(m: &http::HeaderMap, n: &http::header::HeaderName, v: &str)
    -> Result<(), Invalid>
{
    with_header(m, n, move |value| {
        if unicase::Ascii::new(value) != v {
            Err(Invalid(format!("unexpected header value: {}", n)))
        } else {
            Ok(())
        }
    })
}

pub(crate) fn with_header<F, R>(m: &http::HeaderMap, n: &http::header::HeaderName, f: F)
    -> Result<R, Invalid>
where
    F: Fn(&str) -> Result<R, Invalid>
{
    m.get(n)
        .ok_or(Invalid(format!("missing header name: {}", n)))
        .and_then(|value| {
            value.to_str().map_err(|_| Invalid(format!("invalid header: {}", n)))
        })
        .and_then(f)
}


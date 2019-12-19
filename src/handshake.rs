// Copyright (c) 2019 Parity Technologies (UK) Ltd.
//
// Licensed under the Apache License, Version 2.0
// <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0> or the MIT
// license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. All files in the project carrying such notice may not be copied,
// modified, or distributed except according to those terms.

//! Websocket [handshake]s.
//!
//! [handshake]: https://tools.ietf.org/html/rfc6455#section-4

pub mod client;
pub mod server;

use crate::extension::{Param, Extension};
use smallvec::SmallVec;
use std::{io, str};

pub use client::{Client, ServerResponse};
pub use server::{Server, ClientRequest};

// Defined in RFC 6455 and used to generate the `Sec-WebSocket-Accept` header
// in the server handshake response.
const KEY: &[u8] = b"258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

// How many HTTP headers do we support during parsing?
const MAX_NUM_HEADERS: usize = 32;

// Some HTTP headers we need to check during parsing.
const SEC_WEBSOCKET_EXTENSIONS: &str = "Sec-WebSocket-Extensions";
const SEC_WEBSOCKET_PROTOCOL: &str = "Sec-WebSocket-Protocol";

/// Check a set of headers contains a specific one.
fn expect_ascii_header(headers: &[httparse::Header], name: &str, ours: &str) -> Result<(), Error> {
    enum State {
        Init, // Start state
        Name, // Header name found
        Match // Header value matches
    }

    headers.iter()
        .filter(|h| h.name.eq_ignore_ascii_case(name))
        .fold(Ok(State::Init), |result, header| {
            if let Ok(State::Match) = result {
                return result
            }
            if str::from_utf8(header.value)?
                .split(',')
                .any(|v| v.trim().eq_ignore_ascii_case(ours))
            {
                return Ok(State::Match)
            }
            Ok(State::Name)
        })
        .and_then(|state| {
            match state {
                State::Init => Err(Error::HeaderNotFound(name.into())),
                State::Name => Err(Error::UnexpectedHeader(name.into())),
                State::Match => Ok(())
            }
        })
}

/// Pick the first header with the given name and apply the given closure to it.
fn with_first_header<'a, F, R>(headers: &[httparse::Header<'a>], name: &str, f: F) -> Result<R, Error>
where
    F: Fn(&'a [u8]) -> Result<R, Error>
{
    if let Some(h) = headers.iter().find(|h| h.name.eq_ignore_ascii_case(name)) {
        f(h.value)
    } else {
        Err(Error::HeaderNotFound(name.into()))
    }
}

// Configure all extensions with parsed parameters.
fn configure_extensions(extensions: &mut [Box<dyn Extension + Send>], line: &str) -> Result<(), Error> {
    for e in line.split(',') {
        let mut ext_parts = e.split(';');
        if let Some(name) = ext_parts.next() {
            let name = name.trim();
            if let Some(ext) = extensions.iter_mut().find(|x| x.name().eq_ignore_ascii_case(name)) {
                let mut params = SmallVec::<[Param; 4]>::new();
                for p in ext_parts {
                    let mut key_value = p.split('=');
                    if let Some(key) = key_value.next().map(str::trim) {
                        let val = key_value.next().map(|v| v.trim().trim_matches('"'));
                        let mut p = Param::new(key);
                        p.set_value(val);
                        params.push(p)
                    }
                }
                ext.configure(&params).map_err(Error::Extension)?
            }
        }
    }
    Ok(())
}

// Write all extensions to the given buffer.
fn append_extensions<'a, I>(extensions: I, bytes: &mut crate::Buffer)
where
    I: IntoIterator<Item = &'a Box<dyn Extension + Send>>
{
    let mut iter = extensions.into_iter().peekable();

    if iter.peek().is_some() {
        bytes.extend_from_slice(b"\r\nSec-WebSocket-Extensions: ")
    }

    while let Some(e) = iter.next() {
        bytes.extend_from_slice(e.name().as_bytes());
        for p in e.params() {
            bytes.extend_from_slice(b"; ");
            bytes.extend_from_slice(p.name().as_bytes());
            if let Some(v) = p.value() {
                bytes.extend_from_slice(b"=");
                bytes.extend_from_slice(v.as_bytes())
            }
        }
        if iter.peek().is_some() {
            bytes.extend_from_slice(b", ")
        }
    }
}

/// Enumeration of possible handshake errors.
#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// An I/O error has been encountered.
    #[error("i/o error: {0}")]
    Io(#[from] io::Error),

    /// An HTTP version =/= 1.1 was encountered.
    #[error("http version was not 1.1")]
    UnsupportedHttpVersion,

    /// The handshake request was not a GET request.
    #[error("handshake was not a GET request")]
    InvalidRequestMethod,

    /// An HTTP header has not been present.
    #[error("header {0} not found")]
    HeaderNotFound(String),

    /// An HTTP header value was not expected.
    #[error("header {0} had an unexpected value")]
    UnexpectedHeader(String),

    /// The Sec-WebSocket-Accept header value did not match.
    #[error("websocket key mismatch")]
    InvalidSecWebSocketAccept,

    /// The server returned an extension we did not ask for.
    #[error("unsolicited extension returned")]
    UnsolicitedExtension,

    /// The server returned a protocol we did not ask for.
    #[error("unsolicited protocol returned")]
    UnsolicitedProtocol,

    /// An extension produced an error while encoding or decoding.
    #[error("extension error: {0}")]
    Extension(crate::BoxedError),

    /// The HTTP entity could not be parsed successfully.
    #[error("http parser error: {0}")]
    Http(crate::BoxedError),

    /// UTF-8 decoding failed.
    #[error("utf-8 decoding error: {0}")]
    Utf8(#[from] std::str::Utf8Error)
}

#[cfg(test)]
mod tests {
    use super::expect_ascii_header;

    #[test]
    fn header_match() {
        let headers = &[
            httparse::Header { name: "foo", value: b"a,b,c,d" },
            httparse::Header { name: "foo", value: b"x" },
            httparse::Header { name: "foo", value: b"y, z, a" },
            httparse::Header { name: "bar", value: b"xxx" },
            httparse::Header { name: "bar", value: b"sdfsdf 423 42 424" },
            httparse::Header { name: "baz", value: b"123" }
        ];

        assert!(expect_ascii_header(headers, "foo", "a").is_ok());
        assert!(expect_ascii_header(headers, "foo", "b").is_ok());
        assert!(expect_ascii_header(headers, "foo", "c").is_ok());
        assert!(expect_ascii_header(headers, "foo", "d").is_ok());
        assert!(expect_ascii_header(headers, "foo", "x").is_ok());
        assert!(expect_ascii_header(headers, "foo", "y").is_ok());
        assert!(expect_ascii_header(headers, "foo", "z").is_ok());
        assert!(expect_ascii_header(headers, "foo", "a").is_ok());
        assert!(expect_ascii_header(headers, "bar", "xxx").is_ok());
        assert!(expect_ascii_header(headers, "bar", "sdfsdf 423 42 424").is_ok());
        assert!(expect_ascii_header(headers, "baz", "123").is_ok());
        assert!(expect_ascii_header(headers, "baz", "???").is_err());
        assert!(expect_ascii_header(headers, "???", "x").is_err());
    }
}

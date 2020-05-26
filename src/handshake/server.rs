// Copyright (c) 2019 Parity Technologies (UK) Ltd.
//
// Licensed under the Apache License, Version 2.0
// <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0> or the MIT
// license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. All files in the project carrying such notice may not be copied,
// modified, or distributed except according to those terms.

//! Websocket server [handshake].
//!
//! [handshake]: https://tools.ietf.org/html/rfc6455#section-4

use bytes::{Buf, BytesMut};
use crate::{Parsing, extension::Extension};
use crate::connection::{self, Mode};
use futures::prelude::*;
use sha1::{Digest, Sha1};
use std::{mem, str};
use super::{
    Error,
    KEY,
    MAX_NUM_HEADERS,
    SEC_WEBSOCKET_EXTENSIONS,
    SEC_WEBSOCKET_PROTOCOL,
    append_extensions,
    configure_extensions,
    expect_ascii_header,
    with_first_header
};

const BLOCK_SIZE: usize = 8 * 1024;
const SOKETTO_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Websocket handshake client.
#[derive(Debug)]
pub struct Server<'a, T> {
    socket: T,
    /// Protocols the server supports.
    protocols: Vec<&'a str>,
    /// Extensions the server supports.
    extensions: Vec<Box<dyn Extension + Send>>,
    /// Encoding/decoding buffer.
    buffer: BytesMut
}

impl<'a, T: AsyncRead + AsyncWrite + Unpin> Server<'a, T> {
    /// Create a new server handshake.
    pub fn new(socket: T) -> Self {
        Server {
            socket,
            protocols: Vec::new(),
            extensions: Vec::new(),
            buffer: BytesMut::new()
        }
    }

    /// Override the buffer to use for request/response handling.
    pub fn set_buffer(&mut self, b: BytesMut) -> &mut Self {
        self.buffer = b;
        self
    }

    /// Extract the buffer.
    pub fn take_buffer(&mut self) -> BytesMut {
        mem::take(&mut self.buffer)
    }

    /// Add a protocol the server supports.
    pub fn add_protocol(&mut self, p: &'a str) -> &mut Self {
        self.protocols.push(p);
        self
    }

    /// Add an extension the server supports.
    pub fn add_extension(&mut self, e: Box<dyn Extension + Send>) -> &mut Self {
        self.extensions.push(e);
        self
    }

    /// Get back all extensions.
    pub fn drain_extensions(&mut self) -> impl Iterator<Item = Box<dyn Extension + Send>> + '_ {
        self.extensions.drain(..)
    }

    /// Await an incoming client handshake request.
    pub async fn receive_request(&mut self) -> Result<ClientRequest<'a>, Error> {
        self.buffer.clear();
        loop {
            crate::read(&mut self.socket, &mut self.buffer, BLOCK_SIZE).await?;
            if let Parsing::Done { value, offset } = self.decode_request()? {
                self.buffer.advance(offset);
                return Ok(value)
            }
        }
    }

    /// Respond to the client.
    pub async fn send_response(&mut self, r: &Response<'_>) -> Result<(), Error> {
        self.buffer.clear();
        self.encode_response(r);
        self.socket.write_all(&self.buffer).await?;
        self.socket.flush().await?;
        self.buffer.clear();
        Ok(())
    }

    /// Turn this handshake into a [`connection::Builder`].
    pub fn into_builder(mut self) -> connection::Builder<T> {
        let mut builder = connection::Builder::new(self.socket, Mode::Server);
        builder.set_buffer(self.buffer);
        builder.add_extensions(self.extensions.drain(..));
        builder
    }

    /// Get out the inner socket of the server.
    pub fn into_inner(self) -> T {
        self.socket
    }

    // Decode client handshake request.
    fn decode_request(&mut self) -> Result<Parsing<ClientRequest<'a>>, Error> {
        let mut header_buf = [httparse::EMPTY_HEADER; MAX_NUM_HEADERS];
        let mut request = httparse::Request::new(&mut header_buf);

        let offset = match request.parse(self.buffer.as_ref()) {
            Ok(httparse::Status::Complete(off)) => off,
            Ok(httparse::Status::Partial) => return Ok(Parsing::NeedMore(())),
            Err(e) => return Err(Error::Http(Box::new(e)))
        };

        if request.method != Some("GET") {
            return Err(Error::InvalidRequestMethod)
        }
        if request.version != Some(1) {
            return Err(Error::UnsupportedHttpVersion)
        }

        // TODO: Host Validation
        with_first_header(&request.headers, "Host", |_h| Ok(()))?;

        expect_ascii_header(request.headers, "Upgrade", "websocket")?;
        expect_ascii_header(request.headers, "Connection", "upgrade")?;
        expect_ascii_header(request.headers, "Sec-WebSocket-Version", "13")?;

        let ws_key = with_first_header(&request.headers, "Sec-WebSocket-Key", |k| {
            Ok(Vec::from(k))
        })?;

        for h in request.headers.iter()
            .filter(|h| h.name.eq_ignore_ascii_case(SEC_WEBSOCKET_EXTENSIONS))
        {
            configure_extensions(&mut self.extensions, std::str::from_utf8(h.value)?)?
        }

        let mut protocols = Vec::new();
        for p in request.headers.iter()
            .filter(|h| h.name.eq_ignore_ascii_case(SEC_WEBSOCKET_PROTOCOL))
        {
            if let Some(&p) = self.protocols.iter().find(|x| x.as_bytes() == p.value) {
                protocols.push(p)
            }
        }

        Ok(Parsing::Done { value: ClientRequest { ws_key, protocols }, offset })
    }

    // Encode server handshake response.
    fn encode_response(&mut self, response: &Response<'_>) {
        match response {
            Response::Accept { key, protocol } => {
                let mut key_buf = [0; 32];
                let accept_value = {
                    let mut digest = Sha1::new();
                    digest.input(key);
                    digest.input(KEY);
                    let d = digest.result();
                    let n = base64::encode_config_slice(&d, base64::STANDARD, &mut key_buf);
                    &key_buf[.. n]
                };
                self.buffer.extend_from_slice(b"HTTP/1.1 101 Switching Protocols");
                self.buffer.extend_from_slice(b"\r\nServer: soketto-");
                self.buffer.extend_from_slice(SOKETTO_VERSION.as_bytes());
                self.buffer.extend_from_slice(b"\r\nUpgrade: websocket\r\nConnection: upgrade");
                self.buffer.extend_from_slice(b"\r\nSec-WebSocket-Accept: ");
                self.buffer.extend_from_slice(accept_value);
                if let Some(p) = protocol {
                    self.buffer.extend_from_slice(b"\r\nSec-WebSocket-Protocol: ");
                    self.buffer.extend_from_slice(p.as_bytes())
                }
                append_extensions(self.extensions.iter().filter(|e| e.is_enabled()), &mut self.buffer);
                self.buffer.extend_from_slice(b"\r\n\r\n")
            }
            Response::Reject { status_code } => {
                self.buffer.extend_from_slice(b"HTTP/1.1 ");
                let (_, s, reason) =
                    if let Ok(i) = STATUSCODES.binary_search_by_key(status_code, |(n, _, _)| *n) {
                        STATUSCODES[i]
                    } else {
                        (500, "500", "Internal Server Error")
                    };
                self.buffer.extend_from_slice(s.as_bytes());
                self.buffer.extend_from_slice(b" ");
                self.buffer.extend_from_slice(reason.as_bytes());
                self.buffer.extend_from_slice(b"\r\n\r\n")
            }
        }
    }
}

/// Handshake request received from the client.
#[derive(Debug)]
pub struct ClientRequest<'a> {
    ws_key: Vec<u8>,
    protocols: Vec<&'a str>
}

impl<'a> ClientRequest<'a> {
    /// A reference to the nonce.
    pub fn key(&self) -> &[u8] {
        &self.ws_key
    }

    pub fn into_key(self) -> Vec<u8> {
        self.ws_key
    }

    /// The protocols the client is proposing.
    pub fn protocols(&self) -> impl Iterator<Item = &str> {
        self.protocols.iter().cloned()
    }
}

/// Handshake response the server sends back to the client.
#[derive(Debug)]
pub enum Response<'a> {
    /// The server accepts the handshake request.
    Accept {
        key: &'a [u8],
        protocol: Option<&'a str>
    },
    /// The server rejects the handshake request.
    Reject {
        status_code: u16
    }
}

/// Known status codes and their reason phrases.
const STATUSCODES: &[(u16, &str, &str)] = &[
    (100, "100", "Continue"),
    (101, "101", "Switching Protocols"),
    (102, "102", "Processing"),
    (200, "200", "OK"),
    (201, "201", "Created"),
    (202, "202", "Accepted"),
    (203, "203", "Non Authoritative Information"),
    (204, "204", "No Content"),
    (205, "205", "Reset Content"),
    (206, "206", "Partial Content"),
    (207, "207", "Multi-Status"),
    (208, "208", "Already Reported"),
    (226, "226", "IM Used"),
    (300, "300", "Multiple Choices"),
    (301, "301", "Moved Permanently"),
    (302, "302", "Found"),
    (303, "303", "See Other"),
    (304, "304", "Not Modified"),
    (305, "305", "Use Proxy"),
    (307, "307", "Temporary Redirect"),
    (308, "308", "Permanent Redirect"),
    (400, "400", "Bad Request"),
    (401, "401", "Unauthorized"),
    (402, "402", "Payment Required"),
    (403, "403", "Forbidden"),
    (404, "404", "Not Found"),
    (405, "405", "Method Not Allowed"),
    (406, "406", "Not Acceptable"),
    (407, "407", "Proxy Authentication Required"),
    (408, "408", "Request Timeout"),
    (409, "409", "Conflict"),
    (410, "410", "Gone"),
    (411, "411", "Length Required"),
    (412, "412", "Precondition Failed"),
    (413, "413", "Payload Too Large"),
    (414, "414", "URI Too Long"),
    (415, "415", "Unsupported Media Type"),
    (416, "416", "Range Not Satisfiable"),
    (417, "417", "Expectation Failed"),
    (418, "418", "I'm a teapot"),
    (421, "421", "Misdirected Request"),
    (422, "422", "Unprocessable Entity"),
    (423, "423", "Locked"),
    (424, "424", "Failed Dependency"),
    (426, "426", "Upgrade Required"),
    (428, "428", "Precondition Required"),
    (429, "429", "Too Many Requests"),
    (431, "431", "Request Header Fields Too Large"),
    (451, "451", "Unavailable For Legal Reasons"),
    (500, "500", "Internal Server Error"),
    (501, "501", "Not Implemented"),
    (502, "502", "Bad Gateway"),
    (503, "503", "Service Unavailable"),
    (504, "504", "Gateway Timeout"),
    (505, "505", "HTTP Version Not Supported"),
    (506, "506", "Variant Also Negotiates"),
    (507, "507", "Insufficient Storage"),
    (508, "508", "Loop Detected"),
    (510, "510", "Not Extended"),
    (511, "511", "Network Authentication Required")
];


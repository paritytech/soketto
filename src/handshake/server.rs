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

use bytes::{BufMut, BytesMut};
use crate::{Parsing, connection::{Connection, Mode}, extension::Extension};
use futures::prelude::*;
use http::StatusCode;
use log::trace;
use sha1::Sha1;
use smallvec::SmallVec;
use std::str;
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

const BLOCK_SIZE: usize = 8192;
const SOKETTO_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Websocket handshake client.
#[derive(Debug)]
pub struct Server<'a, T> {
    socket: T,
    /// Protocols the server supports.
    protocols: SmallVec<[&'a str; 4]>,
    /// Extensions the server supports.
    extensions: SmallVec<[Box<dyn Extension + Send>; 4]>,
    /// Encoding/decoding buffer
    buffer: BytesMut
}

impl<'a, T: AsyncRead + AsyncWrite + Unpin> Server<'a, T> {
    /// Create a new server handshake.
    pub fn new(socket: T) -> Self {
        Server {
            socket,
            protocols: SmallVec::new(),
            extensions: SmallVec::new(),
            buffer: BytesMut::new()
        }
    }

    pub fn set_buffer(&mut self, b: BytesMut) -> &mut Self {
        self.buffer = b;
        self
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
        self.extensions.drain()
    }

    /// Await an incoming client handshake request.
    pub async fn receive_request(&mut self) -> Result<ClientRequest<'a>, Error> {
        self.buffer.clear();
        loop {
            if !self.buffer.has_remaining_mut() {
                self.buffer.reserve(BLOCK_SIZE)
            }
            unsafe {
                let n = self.socket.read(self.buffer.bytes_mut()).await?;
                self.buffer.advance_mut(n);
                trace!("read {} bytes", n)
            }
            if let Parsing::Done { value, offset } = self.decode_request()? {
                self.buffer.split_to(offset);
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

    /// Turn this handshake into a [`Connection`].
    ///
    /// If `take_over_extensions` is true, the extensions from this
    /// handshake will be set on the `Connection` returned.
    pub fn into_connection(mut self, take_over_extensions: bool) -> Connection<T> {
        let mut c = Connection::new(self.socket, Mode::Server);
        c.set_buffer(self.buffer);
        if take_over_extensions {
            c.add_extensions(self.extensions.drain());
        }
        c
    }

    // Decode client handshake request.
    fn decode_request(&mut self) -> Result<Parsing<ClientRequest<'a>>, Error> {
        let mut header_buf = [httparse::EMPTY_HEADER; MAX_NUM_HEADERS];
        let mut request = httparse::Request::new(&mut header_buf);

        let offset = match request.parse(&self.buffer) {
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

        let mut protocols = SmallVec::new();
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
                    digest.update(key);
                    digest.update(KEY);
                    let d = digest.digest().bytes();
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
                let s = StatusCode::from_u16(*status_code)
                    .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
                self.buffer.extend_from_slice(s.as_str().as_bytes());
                self.buffer.extend_from_slice(b" ");
                self.buffer.extend_from_slice(s.canonical_reason().unwrap_or("N/A").as_bytes());
                self.buffer.extend_from_slice(b"\r\n\r\n")
            }
        }
    }
}

/// Handshake request received from the client.
#[derive(Debug)]
pub struct ClientRequest<'a> {
    ws_key: Vec<u8>,
    protocols: SmallVec<[&'a str; 4]>
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


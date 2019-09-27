// Copyright (c) 2019 Parity Technologies (UK) Ltd.
// Copyright (c) 2016 twist developers
//
// Licensed under the Apache License, Version 2.0
// <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0> or the MIT
// license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. All files in the project carrying such notice may not be copied,
// modified, or distributed except according to those terms.

//! An implementation of the [RFC 6455][rfc6455] websocket protocol.
//!
//! # Examples
//!
//! To begin a websocket connection one first needs to perform a [handshake],
//! either as [client] or [server], in order to upgrade from HTTP.
//! Once successful, the client or server can transition to a [connection]
//! and send and receive textual or binary data.
//!
//! ## Client
//!
//! ```
//! # use async_std::net::TcpStream;
//! # let _: Result<(), soketto::BoxedError> = async_std::task::block_on(async {
//! use soketto::handshake::{Client, ServerResponse};
//!
//! // First, we need to establish a TCP connection.
//! let socket = TcpStream::connect("...").await?;
//!
//! // Then we configure the client handshake.
//! let mut client = Client::new(socket, "...", "/");
//!
//! // And finally we perform the handshake and handle the result.
//! let mut websocket = match client.handshake().await? {
//!     ServerResponse::Accepted { .. } => client.into_connection(),
//!     ServerResponse::Redirect { status_code, location } =>
//!         unimplemented!("reconnect to the location URL"),
//!     ServerResponse::Rejected { status_code } =>
//!         unimplemented!("handle failure")
//! };
//!
//! // Over the established websocket connection we can send
//! websocket.send_text(&mut "some text".into()).await?;
//! websocket.send_text(&mut "some more text".into()).await?;
//! websocket.flush().await?;
//!
//! // ... and receive data.
//! let (answer, is_text) = websocket.receive().await?;
//!
//! # Ok(())
//! # });
//!
//! ```
//!
//! ## Server
//!
//! ```
//! # use async_std::{net::TcpListener, prelude::*};
//! # let _: Result<(), soketto::BoxedError> = async_std::task::block_on(async {
//! use soketto::handshake::{Server, ClientRequest, server::Response};
//!
//! // First, we listen for incoming connections.
//! let listener = TcpListener::bind("...").await?;
//! let mut incoming = listener.incoming();
//!
//! while let Some(socket) = incoming.next().await {
//!     // For each incoming connection we perform a handshake.
//!     let mut server = Server::new(socket?);
//!
//!     let websocket_key = {
//!         let req = server.receive_request().await?;
//!         req.into_key()
//!     };
//!
//!     // Here we accept the client unconditionally.
//!     let accept = Response::Accept { key: &websocket_key, protocol: None };
//!     server.send_response(&accept).await?;
//!
//!     // And we can finally transition to a websocket connection.
//!     let mut websocket = server.into_connection();
//!     let (mut message, is_text) = websocket.receive().await?;
//!     if is_text {
//!         websocket.send_text(&mut message).await?
//!     } else {
//!         websocket.send_binary(&mut message).await?
//!     }
//!     websocket.close().await?;
//! }
//!
//! # Ok(())
//! # });
//!
//! ```
//! [client]: handshake::Client
//! [server]: handshake::Server
//! [connection]: connection::Connection
//! [rfc6455]: https://tools.ietf.org/html/rfc6455
//! [handshake]: https://tools.ietf.org/html/rfc6455#section-4

pub mod base;
pub mod extension;
pub mod handshake;
pub mod connection;

use bytes::{BufMut, BytesMut};
use futures::io::{AsyncRead, AsyncReadExt};

pub type BoxedError = Box<dyn std::error::Error + Send + Sync>;

/// A parsing result.
#[derive(Debug, Clone)]
pub enum Parsing<T, N = ()> {
    /// Parsing completed.
    Done {
        /// The parsed value.
        value: T,
        /// The offset into the byte slice that has been consumed.
        offset: usize
    },
    /// Parsing is incomplete and needs more data.
    NeedMore(N)
}

/// Helper function to allow casts from `usize` to `u64` only on platforms
/// where the sizes are guaranteed to fit.
#[cfg(any(target_pointer_width = "32", target_pointer_width = "64"))]
pub(crate) fn as_u64(a: usize) -> u64 {
    a as u64
}

/// Helper to read from an `AsyncRead` resource into some buffer.
pub(crate) async fn read<R, E>(r: &mut R, b: &mut BytesMut) -> Result<(), E>
where
    R: AsyncRead + Unpin,
    E: From<std::io::Error>
{
    unsafe {
        // `bytes_mut()` is marked unsafe because it returns a
        // reference to uninitialised memory. Since we do not
        // read this memory and initialise it if necessary,
        // usage is safe here.
        //
        // `advance_mut()` is marked unsafe because it can not
        // know if the memory is safe to read. Since we only
        // advance for as many bytes as we have read, usage is
        // safe here.
        initialise(b.bytes_mut());
        let n = r.read(b.bytes_mut()).await?;
        b.advance_mut(n);
        log::trace!("read {} bytes", n)
    }
    Ok(())
}

/// Helper to initialise a slice by filling it with 0s.
fn initialise(m: &mut [u8]) {
    if cfg!(feature = "read_with_uninitialised_memory") {
        return ()
    }
    unsafe {
        std::ptr::write_bytes(m.as_mut_ptr(), 0, m.len())
    }
}


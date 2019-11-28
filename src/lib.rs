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
//! To begin a websocket connection one first needs to perform a [handshake],
//! either as [client] or [server], in order to upgrade from HTTP.
//! Once successful, the client or server can transition to a connection,
//! i.e. a [Sender]/[Receiver] pair and send and receive textual or
//! binary data.
//!
//! **Note**: While it is possible to only receive websocket messages it is
//! not possible to only send websocket messages. Receiving data is required
//! in order to react to control frames such as PING or CLOSE. While those will be
//! answered transparently they have to be received in the first place, so
//! calling [`connection::Receiver::receive`] is imperative.
//!
//! **Note**: None of the `async` methods are safe to cancel so their `Future`s
//! must not be dropped unless they return `Poll::Ready`.
//!
//! # Client example
//!
//! ```no_run
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
//! let (mut sender, mut receiver) = match client.handshake().await? {
//!     ServerResponse::Accepted { .. } => client.into_builder().finish(),
//!     ServerResponse::Redirect { status_code, location } => unimplemented!("follow location URL"),
//!     ServerResponse::Rejected { status_code } => unimplemented!("handle failure")
//! };
//!
//! // Over the established websocket connection we can send
//! sender.send_text("some text").await?;
//! sender.send_text("some more text").await?;
//! sender.flush().await?;
//!
//! // ... and receive data.
//! let data = receiver.receive_data().await?;
//!
//! # Ok(())
//! # });
//!
//! ```
//!
//! # Server example
//!
//! ```no_run
//! # use async_std::{net::TcpListener, prelude::*};
//! # let _: Result<(), soketto::BoxedError> = async_std::task::block_on(async {
//! use soketto::{handshake::{Server, ClientRequest, server::Response}};
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
//!     let (mut sender, mut receiver) = server.into_builder().finish();
//!
//!     let data = receiver.receive_data().await?;
//!
//!     if data.is_text() {
//!         sender.send_text(std::str::from_utf8(data.as_ref())?).await?
//!     } else {
//!         sender.send_binary(data.as_ref()).await?
//!     }
//!
//!     sender.close().await?;
//! }
//!
//! # Ok(())
//! # });
//!
//! ```
//! [client]: handshake::Client
//! [server]: handshake::Server
//! [Sender]: connection::Sender
//! [Receiver]: connection::Receiver
//! [rfc6455]: https://tools.ietf.org/html/rfc6455
//! [handshake]: https://tools.ietf.org/html/rfc6455#section-4

pub mod base;
pub mod data;
pub mod extension;
pub mod handshake;
pub mod connection;

use bytes::{BufMut, BytesMut};
use futures::io::{AsyncRead, AsyncReadExt};
use std::{io, mem::{self, MaybeUninit}, ptr};

pub use connection::{Mode, Receiver, Sender};

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

/// A buffer type used for implementing `Extension`s.
#[derive(Debug)]
pub enum Storage<'a> {
    /// A read-only shared byte slice.
    Shared(&'a [u8]),
    /// A mutable byte slice.
    Unique(&'a mut [u8]),
    /// An owned byte buffer.
    Owned(BytesMut)
}

impl AsRef<[u8]> for Storage<'_> {
    fn as_ref(&self) -> &[u8] {
        match self {
            Storage::Shared(d) => d,
            Storage::Unique(d) => d,
            Storage::Owned(b) => b.as_ref()
        }
    }
}

/// Helper function to allow casts from `usize` to `u64` only on platforms
/// where the sizes are guaranteed to fit.
#[cfg(any(target_pointer_width = "32", target_pointer_width = "64"))]
const fn as_u64(a: usize) -> u64 {
    a as u64
}

/// Wrapper around `BytesMut` with a safe API.
#[derive(Debug)]
pub(crate) struct Buffer(BytesMut);

impl Buffer {
    /// Create a fresh empty buffer.
    pub(crate) fn new() -> Self {
        Buffer(BytesMut::new())
    }

    /// Create a fresh empty buffer.
    pub(crate) fn from(b: BytesMut) -> Self {
        let mut this = Buffer(b);
        // We do not know if the capacity of `b` is fully initialised
        // so we do it ourselves.
        this.init_bytes_mut();
        this
    }

    /// Buffer length in bytes.
    pub(crate) fn len(&self) -> usize {
        self.0.len()
    }

    /// The remaining write capacity of this buffer.
    pub(crate) fn remaining_mut(&self) -> usize {
        self.0.capacity() - self.0.len()
    }

    /// Clear this buffer.
    pub(crate) fn clear(&mut self) {
        self.0.clear()
    }

    /// Set `self` to `self[n ..]` and return `self[.. n]`.
    pub(crate) fn split_to(&mut self, n: usize) -> Self {
        Buffer(self.0.split_to(n))
    }

    /// Return all bytes from `self`, leaving it empty.
    #[cfg(feature = "deflate")]
    pub(crate) fn take(&mut self) -> Self {
        self.split_to(self.0.len())
    }

    /// Shorten the buffer to the given len.
    #[cfg(feature = "deflate")]
    pub(crate) fn truncate(&mut self, len: usize) {
        self.0.truncate(len)
    }

    /// Extract the underlying storage bytes.
    pub(crate) fn into_bytes(self) -> BytesMut {
        self.0
    }

    /// Clear this buffer.
    pub(crate) fn extend_from_slice(&mut self, slice: &[u8]) {
        self.0.extend_from_slice(slice)
    }

    /// Reserve and initialise more capacity.
    pub(crate) fn reserve(&mut self, additional: usize) {
        let old = self.0.capacity();
        self.0.reserve(additional);
        let new = self.0.capacity();
        if new > old {
            self.init_bytes_mut()
        }
    }

    /// Get a mutable handle to the remaining write capacity.
    pub(crate) fn bytes_mut(&mut self) -> &mut [u8] {
        let b = self.0.bytes_mut();
        unsafe {
            // Safe because `reserve` always initialises memory.
            mem::transmute::<&mut [MaybeUninit<u8>], &mut [u8]>(b)
        }
    }

    /// Increment the buffer length by `n` bytes.
    pub(crate) fn advance_mut(&mut self, n: usize) {
        assert!(n <= self.remaining_mut(), "{} > {}", n, self.remaining_mut());
        unsafe {
            // Safe because we have established that `n` does not exceed
            // the remaining capacity.
            self.0.advance_mut(n)
        }
    }

    /// Write 0s into the remaining write capacity.
    fn init_bytes_mut(&mut self) {
        let b = self.0.bytes_mut();
        unsafe {
            // Safe because we never read from `b` and stay within
            // the boundaries of `b` when writing.
            ptr::write_bytes(b.as_mut_ptr(), 0, b.len())
        }
    }

    /// Fill the buffer from the given `AsyncRead` impl.
    pub(crate) async fn read_from<R>(&mut self, reader: &mut R) -> io::Result<()>
    where
        R: AsyncRead + Unpin
    {
        let b = self.bytes_mut();
        debug_assert!(!b.is_empty());
        let n = reader.read(b).await?;
        if n == 0 {
            return Err(std::io::ErrorKind::UnexpectedEof.into())
        }
        self.advance_mut(n);
        log::trace!("read {} bytes", n);
        Ok(())
    }
}

impl AsRef<[u8]> for Buffer {
    fn as_ref(&self) -> &[u8] {
        self.0.as_ref()
    }
}

impl AsMut<[u8]> for Buffer {
    fn as_mut(&mut self) -> &mut [u8] {
        self.0.as_mut()
    }
}

/// Return all bytes from the given `BytesMut`, leaving it empty.
pub(crate) fn take(bytes: &mut BytesMut) -> BytesMut {
    bytes.split_to(bytes.len())
}


// Copyright (c) 2019 Parity Technologies (UK) Ltd.
//
// Licensed under the Apache License, Version 2.0
// <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0> or the MIT
// license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. All files in the project carrying such notice may not be copied,
// modified, or distributed except according to those terms.

//! Make futures io traits compatible with tokio_io.
//!
//! I.e. we map `futures::io::AsyncRead` to `tokio_io::AsyncRead` and
//! `futures::io::AsyncWrite` to `tokio_io::AsyncWrite`.

use futures::io::{AsyncRead, AsyncWrite};
use std::{io, pin::Pin, task::{Context, Poll}};

#[derive(Debug)]
pub(crate) struct AioCompat<T>(pub(crate) T);

impl<T: Unpin> AioCompat<T> {
    fn inner(self: Pin<&mut Self>) -> Pin<&mut T> {
        Pin::new(&mut self.get_mut().0)
    }
}

impl<T: AsyncRead + Unpin> tokio_io::AsyncRead for AioCompat<T> {
    fn poll_read(self: Pin<&mut Self>, cx: &mut Context, buf: &mut [u8]) -> Poll<io::Result<usize>> {
        self.inner().poll_read(cx, buf)
    }
}

impl<T: AsyncWrite + Unpin> tokio_io::AsyncWrite for AioCompat<T> {
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context, buf: &[u8]) -> Poll<io::Result<usize>> {
        self.inner().poll_write(cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context) -> Poll<io::Result<()>> {
        self.inner().poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context) -> Poll<io::Result<()>> {
        self.inner().poll_close(cx)
    }
}


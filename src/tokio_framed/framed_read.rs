// Copyright (c) 2019 Tokio Contributors
//
// Permission is hereby granted, free of charge, to any
// person obtaining a copy of this software and associated
// documentation files (the "Software"), to deal in the
// Software without restriction, including without
// limitation the rights to use, copy, modify, merge,
// publish, distribute, sublicense, and/or sell copies of
// the Software, and to permit persons to whom the Software
// is furnished to do so, subject to the following
// conditions:
//
// The above copyright notice and this permission notice
// shall be included in all copies or substantial portions
// of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF
// ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED
// TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A
// PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT
// SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY
// CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION
// OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR
// IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
// DEALINGS IN THE SOFTWARE.

use bytes::BytesMut;
use futures::{try_ready, Async, Poll, Stream};
use log::trace;
use tokio_codec::Decoder;
use tokio_io::AsyncRead;

pub struct FramedRead2<T> {
    inner: T,
    eof: bool,
    is_readable: bool,
    buffer: BytesMut,
}

const INITIAL_CAPACITY: usize = 8 * 1024;

// ===== impl FramedRead2 =====

pub fn framed_read2<T>(inner: T) -> FramedRead2<T> {
    FramedRead2 {
        inner: inner,
        eof: false,
        is_readable: false,
        buffer: BytesMut::with_capacity(INITIAL_CAPACITY),
    }
}

pub fn framed_read2_with_buffer<T>(inner: T, mut buf: BytesMut) -> FramedRead2<T> {
    if buf.capacity() < INITIAL_CAPACITY {
        let bytes_to_reserve = INITIAL_CAPACITY - buf.capacity();
        buf.reserve(bytes_to_reserve);
    }
    FramedRead2 {
        inner: inner,
        eof: false,
        is_readable: buf.len() > 0,
        buffer: buf,
    }
}

impl<T> FramedRead2<T> {
    pub fn get_ref(&self) -> &T {
        &self.inner
    }

    pub fn into_inner(self) -> T {
        self.inner
    }

    pub fn into_parts(self) -> (T, BytesMut) {
        (self.inner, self.buffer)
    }

    pub fn get_mut(&mut self) -> &mut T {
        &mut self.inner
    }
}

impl<T> Stream for FramedRead2<T>
where
    T: AsyncRead + Decoder,
{
    type Item = T::Item;
    type Error = T::Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        loop {
            // Repeatedly call `decode` or `decode_eof` as long as it is
            // "readable". Readable is defined as not having returned `None`. If
            // the upstream has returned EOF, and the decoder is no longer
            // readable, it can be assumed that the decoder will never become
            // readable again, at which point the stream is terminated.
            if self.is_readable {
                if self.eof {
                    let frame = self.inner.decode_eof(&mut self.buffer)?;
                    return Ok(Async::Ready(frame));
                }

                trace!("attempting to decode a frame");

                if let Some(frame) = self.inner.decode(&mut self.buffer)? {
                    trace!("frame decoded from buffer");
                    return Ok(Async::Ready(Some(frame)));
                }

                self.is_readable = false;
            }

            assert!(!self.eof);

            // Otherwise, try to read more data and try again. Make sure we've
            // got room for at least one byte to read to ensure that we don't
            // get a spurious 0 that looks like EOF
            self.buffer.reserve(1);
            if 0 == try_ready!(self.inner.read_buf(&mut self.buffer)) {
                self.eof = true;
            }

            self.is_readable = true;
        }
    }
}

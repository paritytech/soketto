// Copyright (c) 2019 Parity Technologies (UK) Ltd.
//
// Licensed under the Apache License, Version 2.0
// <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0> or the MIT
// license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. All files in the project carrying such notice may not be copied,
// modified, or distributed except according to those terms.

//! A persistent websocket connection after the handshake phase.

use bytes::{BufMut, BytesMut};
use crate::{Parsing, base::{self, Header, OpCode}, extension::Extension};
use log::{debug, trace, warn};
use futures::prelude::*;
use smallvec::SmallVec;
use std::{fmt, io};

const BLOCK_SIZE: usize = 8 * 1024;

/// Is the [`Connection`] used by a client or server?
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Client-side of a connection (implies masking of payload data).
    Client,
    /// Server-side of a connection.
    Server
}

impl Mode {
    pub fn is_client(self) -> bool {
        if let Mode::Client = self {
            true
        } else {
            false
        }
    }

    pub fn is_server(self) -> bool {
        !self.is_client()
    }
}

/// A persistent websocket connection.
#[derive(Debug)]
pub struct Connection<T> {
    mode: Mode,
    socket: T,
    codec: base::Codec,
    extensions: SmallVec<[Box<dyn Extension + Send>; 4]>,
    validate_utf8: bool,
    is_closed: bool,
    rbuffer: BytesMut,
    wbuffer: BytesMut,
    message: BytesMut,
    max_message_size: usize
}

impl<T: AsyncRead + AsyncWrite + Unpin> Connection<T> {
    /// Create a new `Connection` from the given socket.
    pub fn new(socket: T, mode: Mode) -> Self {
        Connection {
            mode,
            socket,
            codec: base::Codec::default(),
            extensions: SmallVec::new(),
            validate_utf8: false,
            is_closed: false,
            rbuffer: BytesMut::new(),
            wbuffer: BytesMut::new(),
            message: BytesMut::new(),
            max_message_size: 256 * 1024 * 1024
        }
    }

    /// Set a custom buffer to use.
    pub fn set_buffer(&mut self, b: BytesMut) -> &mut Self {
        self.rbuffer = b;
        self
    }

    /// Extract the internal buffer bytes.
    pub fn take_buffer(&mut self) -> BytesMut {
        self.rbuffer.take()
    }

    /// Add extensions to this connection.
    ///
    /// Only enabled extensions will be considered.
    pub fn add_extensions<I>(&mut self, extensions: I) -> &mut Self
    where
        I: IntoIterator<Item = Box<dyn Extension + Send>>
    {
        for e in extensions.into_iter().filter(|e| e.is_enabled()) {
            debug!("using extension: {}", e.name());
            self.codec.add_reserved_bits(e.reserved_bits());
            self.extensions.push(e)
        }
        self
    }

    /// Set the maximum size of a (fragmented) message.
    ///
    /// Message fragments will be buffered and concatenated up to this value.
    pub fn set_max_message_size(&mut self, max: usize) -> &mut Self {
        self.max_message_size = max;
        self
    }

    /// Toggle UTF-8 check for incoming text messages.
    pub fn validate_utf8(&mut self, value: bool) -> &mut Self {
        self.validate_utf8 = value;
        self
    }

    /// Send some binary data over this connection.
    pub async fn send_binary(&mut self, data: &mut BytesMut) -> Result<(), Error> {
        let mut header = Header::new(OpCode::Binary);
        self.send(&mut header, data).await?;
        Ok(())
    }

    /// Send some text data over this connection.
    pub async fn send_text(&mut self, data: &mut BytesMut) -> Result<(), Error> {
        debug_assert!(std::str::from_utf8(&data).is_ok());
        let mut header = Header::new(OpCode::Text);
        self.send(&mut header, data).await?;
        Ok(())
    }

    /// Flush the socket buffer.
    pub async fn flush(&mut self) -> Result<(), Error> {
        trace!("flushing connection");
        if self.is_closed {
            return Ok(())
        }
        if !self.wbuffer.is_empty() {
            self.socket.write_all(&self.wbuffer).await?;
            trace!("flushed {} bytes", self.wbuffer.len());
            self.wbuffer.clear()
        }
        self.socket.flush().await?;
        Ok(())
    }

    /// Send a close message and close the connection.
    pub async fn close(&mut self) -> Result<(), Error> {
        trace!("closing connection");
        if self.is_closed {
            return Ok(())
        }
        let mut header = Header::new(OpCode::Close);
        let mut code = 1000_u16.to_be_bytes(); // 1000 = normal closure
        self.write(&mut header, &mut code[..]).await?;
        self.flush().await?;
        self.socket.close().await?;
        self.is_closed = true;
        Ok(())
    }

    /// Send arbitrary websocket frames.
    ///
    /// Before sending, extensions will be applied to header and payload data.
    async fn send(&mut self, header: &mut Header, data: &mut BytesMut) -> Result<(), Error> {
        if self.is_closed {
            debug!("can not send, connection is closed");
            return Err(Error::Closed)
        }
        for e in &mut self.extensions {
            trace!("encoding with extension: {}", e.name());
            e.encode(header, data).map_err(Error::Extension)?
        }
        self.write(header, data).await?;
        Ok(())
    }

    /// Write final header and payload data to socket.
    ///
    /// The data will be masked if necessary.
    /// No extensions will be applied to header and payload data.
    async fn write(&mut self, header: &mut Header, data: &mut [u8]) -> Result<(), Error> {
        if self.mode.is_client() {
            header.set_masked(true);
            header.set_mask(rand::random());
            self.codec.apply_mask(&header, data)
        }
        header.set_payload_len(data.len());
        trace!("send: {}", header);
        if self.wbuffer.len() > BLOCK_SIZE {
            self.socket.write_all(&self.wbuffer).await?;
            trace!("wrote {} bytes", self.wbuffer.len());
            self.wbuffer.clear()
        }
        let header_bytes = self.codec.encode_header(&header);
        self.wbuffer.extend_from_slice(header_bytes);
        self.wbuffer.extend_from_slice(data);
        Ok(())
    }

    /// Receive the next websocket message.
    ///
    /// Fragmented messages will be concatenated and returned as on block.
    /// The `bool` indicates if the data is textual (if `true`) or binary
    /// (if `false`). If `Connection::validate_utf8` is `true` textual data
    /// is checked for well-formed UTF-8 encoding before returned.
    pub async fn receive(&mut self) -> Result<(BytesMut, bool), Error> {
        let mut first_fragment_opcode = None;
        loop {
            if self.is_closed {
                debug!("can not receive, connection is closed");
                return Err(Error::Closed)
            }

            let mut header = self.receive_header().await?;
            trace!("recv: {}", header);

            // Handle control frames.
            if header.opcode().is_control() {
                debug_assert!(header.payload_len() < 126); // ensured by `base::Codec`
                self.read_buffer(&header).await?;
                let mut data = self.rbuffer.split_to(header.payload_len());
                self.codec.apply_mask(&header, &mut data);
                self.on_control(&header, &mut data).await?;
                continue
            }

            // Check if total message does not exceed maximum.
            if header.payload_len() + self.message.len() > self.max_message_size {
                warn!("accumulated message length exceeds maximum");
                return Err(Error::MessageTooLarge {
                    current: self.message.len() + header.payload_len(),
                    maximum: self.max_message_size
                })
            }

            self.read_buffer(&header).await?;
            self.codec.apply_mask(&header, &mut self.rbuffer[.. header.payload_len()]);
            self.message.unsplit(self.rbuffer.split_to(header.payload_len()));

            match (header.is_fin(), header.opcode()) {
                (false, OpCode::Continue) => { // Intermediate message fragment.
                    if first_fragment_opcode.is_none() {
                        debug!("continue frame while not processing message fragments");
                        return Err(Error::UnexpectedOpCode(OpCode::Continue))
                    }
                    continue
                }
                (false, oc) => { // Initial message fragment.
                    if first_fragment_opcode.is_some() {
                        debug!("initial fragment while already processing a fragmented message");
                        return Err(Error::UnexpectedOpCode(oc))
                    }
                    first_fragment_opcode = Some(oc);
                    self.decode_with_extensions(&mut header)?;
                    continue
                }
                (true, OpCode::Continue) => { // Last message fragment.
                    if let Some(oc) = first_fragment_opcode.take() {
                        header.set_payload_len(self.message.len());
                        trace!("last fragement: accumulated length = {} bytes", self.message.len());
                        self.decode_with_extensions(&mut header)?;
                        header.set_opcode(oc);
                    } else {
                        debug!("last continue frame while not processing message fragments");
                        return Err(Error::UnexpectedOpCode(OpCode::Continue))
                    }
                }
                (true, oc) => { // Regular non-fragmented message.
                    if first_fragment_opcode.is_some() {
                        debug!("regular message in the middle of fragmented message processing");
                        return Err(Error::UnexpectedOpCode(oc))
                    }
                    self.decode_with_extensions(&mut header)?
                }
            }

            let is_text = header.opcode() == OpCode::Text;

            if is_text && self.validate_utf8 {
                std::str::from_utf8(&self.message)?;
            }

            return Ok((self.message.take(), is_text))
        }
    }

    /// Read the next frame header.
    async fn receive_header(&mut self) -> Result<Header, Error> {
        loop {
            match self.codec.decode_header(&self.rbuffer)? {
                Parsing::Done { value: header, offset } => {
                    self.rbuffer.split_to(offset);
                    return Ok(header)
                }
                Parsing::NeedMore(n) => {
                    self.rbuffer.reserve(n);
                    unsafe {
                        let n = self.socket.read(self.rbuffer.bytes_mut()).await?;
                        self.rbuffer.advance_mut(n);
                        trace!("read {} bytes", n)
                    }
                }
            }
        }
    }

    /// Read more data into read buffer if necessary.
    async fn read_buffer(&mut self, header: &Header) -> Result<(), Error> {
        if header.payload_len() > self.rbuffer.len() {
            let to_read = header.payload_len() - self.rbuffer.len();
            self.rbuffer.reserve(to_read);
            unsafe {
                self.socket.read_exact(&mut self.rbuffer.bytes_mut()[.. to_read]).await?;
                self.rbuffer.advance_mut(to_read);
                trace!("read {} bytes", to_read)
            }
        }
        Ok(())
    }

    /// Answer incoming control frames.
    async fn on_control(&mut self, header: &Header, data: &mut BytesMut) -> Result<(), Error> {
        debug_assert_eq!(data.len(), header.payload_len());
        match header.opcode() {
            OpCode::Ping => {
                let mut answer = Header::new(OpCode::Pong);
                self.write(&mut answer, data).await?;
                Ok(())
            }
            OpCode::Pong => Ok(()),
            OpCode::Close => {
                let (mut header, code) = close_answer(data)?;
                if let Some(c) = code {
                    let mut data = c.to_be_bytes();
                    self.write(&mut header, &mut data[..]).await?
                } else {
                    self.write(&mut header, &mut []).await?
                }
                self.flush().await?;
                self.socket.close().await?;
                self.is_closed = true;
                Ok(())
            }
            OpCode::Binary
            | OpCode::Text
            | OpCode::Continue
            | OpCode::Reserved3
            | OpCode::Reserved4
            | OpCode::Reserved5
            | OpCode::Reserved6
            | OpCode::Reserved7
            | OpCode::Reserved11
            | OpCode::Reserved12
            | OpCode::Reserved13
            | OpCode::Reserved14
            | OpCode::Reserved15 => Err(Error::UnexpectedOpCode(header.opcode()))
        }
    }

    /// Apply all extensions to the given header and the internal message buffer.
    fn decode_with_extensions(&mut self, header: &mut Header) -> Result<(), Error> {
        for e in &mut self.extensions {
            trace!("decoding with extension: {}", e.name());
            e.decode(header, &mut self.message).map_err(Error::Extension)?
        }
        Ok(())
    }
}

/// Create a close frame based on the given data.
fn close_answer(data: &[u8]) -> Result<(Header, Option<u16>), Error> {
    let answer = Header::new(OpCode::Close);
    if data.len() < 2 {
        return Ok((answer, None))
    }
    std::str::from_utf8(&data[2 ..])?; // check reason is properly encoded
    let code = u16::from_be_bytes([data[0], data[1]]);
    match code {
        | 1000 ..= 1003
        | 1007 ..= 1011
        | 1015
        | 3000 ..= 4999 => Ok((answer, Some(code))), // acceptable codes
        _               => Ok((answer, Some(1002))) // invalid code => protocol error (1002)
    }
}

// Connection error type //////////////////////////////////////////////////////////////////////////

/// Connection error cases.
#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    /// The base codec errored.
    Codec(base::Error),
    /// An extension produced an error while encoding or decoding.
    Extension(crate::BoxedError),
    /// An unexpected opcode was encountered.
    UnexpectedOpCode(OpCode),
    /// A close reason was not correctly UTF-8 encoded.
    Utf8(std::str::Utf8Error),
    /// The total message payload data size exceeds the configured maximum.
    MessageTooLarge { current: usize, maximum: usize },
    /// The connection is closed.
    Closed,

    #[doc(hidden)]
    __Nonexhaustive

}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::Io(e) => write!(f, "i/o error: {}", e),
            Error::Codec(e) => write!(f, "codec error: {}", e),
            Error::Extension(e) => write!(f, "extension error: {}", e),
            Error::UnexpectedOpCode(c) => write!(f, "unexpected opcode: {}", c),
            Error::Utf8(e) => write!(f, "utf-8 error: {}", e),
            Error::MessageTooLarge { current, maximum } =>
                write!(f, "message to large: len >= {}, maximum = {}", current, maximum),
            Error::Closed => f.write_str("connection closed"),
            Error::__Nonexhaustive => f.write_str("__Nonexhaustive")
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io(e) => Some(e),
            Error::Codec(e) => Some(e),
            Error::Extension(e) => Some(&**e),
            Error::Utf8(e) => Some(e),
            Error::UnexpectedOpCode(_)
            | Error::MessageTooLarge {..}
            | Error::Closed
            | Error::__Nonexhaustive => None
        }
    }
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Error::Io(e)
    }
}

impl From<base::Error> for Error {
    fn from(e: base::Error) -> Self {
        Error::Codec(e)
    }
}

impl From<std::str::Utf8Error> for Error {
    fn from(e: std::str::Utf8Error) -> Self {
        Error::Utf8(e)
    }
}

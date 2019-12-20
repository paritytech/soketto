// Copyright (c) 2019 Parity Technologies (UK) Ltd.
//
// Licensed under the Apache License, Version 2.0
// <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0> or the MIT
// license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. All files in the project carrying such notice may not be copied,
// modified, or distributed except according to those terms.

//! A persistent websocket connection after the handshake phase, represented
//! as a [`Sender`] and [`Receiver`] pair.

use bytes::BytesMut;
use crate::{Storage, Parsing, base::{self, Header, MAX_HEADER_SIZE, OpCode}, extension::Extension};
use crate::data::{ByteSlice125, Data, Incoming};
use futures::{io::{BufWriter, ReadHalf, WriteHalf}, lock::BiLock, prelude::*, stream};
use smallvec::SmallVec;
use static_assertions::const_assert;
use std::io;

/// Allocation block size.
const BLOCK_SIZE: usize = 8 * 1024;
/// Write buffer capacity.
const WRITE_BUFFER_SIZE: usize = 64 * 1024;
/// Accumulated max. size of a complete message.
const MAX_MESSAGE_SIZE: usize = 256 * 1024 * 1024;
/// Max. size of a single message frame.
const MAX_FRAME_SIZE: usize = MAX_MESSAGE_SIZE;

/// Is the connection used by a client or server?
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

/// The sending half of a connection.
#[derive(Debug)]
pub struct Sender<T> {
    mode: Mode,
    codec: base::Codec,
    writer: BiLock<BufWriter<WriteHalf<T>>>,
    buffer: Vec<u8>,  // mask buffer
    extensions: BiLock<SmallVec<[Box<dyn Extension + Send>; 4]>>,
    has_extensions: bool
}

/// The receiving half of a connection.
#[derive(Debug)]
pub struct Receiver<T> {
    mode: Mode,
    codec: base::Codec,
    reader: ReadHalf<T>,
    writer: BiLock<BufWriter<WriteHalf<T>>>,
    extensions: BiLock<SmallVec<[Box<dyn Extension + Send>; 4]>>,
    has_extensions: bool,
    buffer: crate::Buffer, // read buffer
    message: BytesMut, // message buffer (concatenated fragment payloads)
    mask_buffer: Vec<u8>,
    max_message_size: usize,
    is_closed: bool
}

/// A connection builder.
///
/// Allows configuring certain parameters and extensions before
/// creating the [`Sender`]/[`Receiver`] pair that represents the
/// connection.
#[derive(Debug)]
pub struct Builder<T> {
    mode: Mode,
    socket: T,
    codec: base::Codec,
    extensions: SmallVec<[Box<dyn Extension + Send>; 4]>,
    buffer: crate::Buffer,
    max_message_size: usize
}

impl<T: AsyncRead + AsyncWrite + Unpin> Builder<T> {
    /// Create a new `Builder` from the given async I/O resource and mode.
    ///
    /// **Note**: Use this type only after a successful handshake
    /// (cf. [`Client::into_builder`][1] and [`Server::into_builder`][2]
    /// for examples).
    ///
    /// [1]: crate::handshake::Client::into_builder
    /// [2]: crate::handshake::Server::into_builder
    pub fn new(socket: T, mode: Mode) -> Self {
        let mut codec = base::Codec::default();
        codec.set_max_data_size(MAX_FRAME_SIZE);
        Builder {
            mode,
            socket,
            codec,
            extensions: SmallVec::new(),
            buffer: crate::Buffer::new(),
            max_message_size: MAX_MESSAGE_SIZE
        }
    }

    /// Set a custom buffer to use.
    pub fn set_buffer(&mut self, b: BytesMut) {
        self.buffer = crate::Buffer::from(b)
    }

    /// Add extensions to use with this connection.
    ///
    /// Only enabled extensions will be considered.
    pub fn add_extensions<I>(&mut self, extensions: I)
    where
        I: IntoIterator<Item = Box<dyn Extension + Send>>
    {
        for e in extensions.into_iter().filter(|e| e.is_enabled()) {
            log::debug!("using extension: {}", e.name());
            self.codec.add_reserved_bits(e.reserved_bits());
            self.extensions.push(e)
        }
    }

    /// Set the maximum size of a complete message.
    ///
    /// Message fragments will be buffered and concatenated up to this value,
    /// i.e. the sum of all message frames payload lengths will not be greater
    /// than this maximum. However, extensions may increase the total message
    /// size further, e.g. by decompressing the payload data.
    pub fn set_max_message_size(&mut self, max: usize) {
        self.max_message_size = max
    }

    /// Set the maximum size of a single websocket frame payload.
    pub fn set_max_frame_size(&mut self, max: usize) {
        self.codec.set_max_data_size(max);
    }

    /// Create a configured [`Sender`]/[`Receiver`] pair.
    pub fn finish(self) -> (Sender<T>, Receiver<T>) {
        let (rhlf, whlf) = self.socket.split();
        let (wrt1, wrt2) = BiLock::new(BufWriter::with_capacity(WRITE_BUFFER_SIZE, whlf));
        let has_extensions = !self.extensions.is_empty();
        let (ext1, ext2) = BiLock::new(self.extensions);

        let recv = Receiver {
            mode: self.mode,
            reader: rhlf,
            writer: wrt1,
            codec: self.codec.clone(),
            extensions: ext1,
            has_extensions,
            buffer: self.buffer,
            message: BytesMut::new(),
            mask_buffer: Vec::new(),
            max_message_size: self.max_message_size,
            is_closed: false
        };

        let send = Sender {
            mode: self.mode,
            writer: wrt2,
            buffer: Vec::new(),
            codec: self.codec,
            extensions: ext2,
            has_extensions
        };

        (send, recv)
    }
}

impl<T: AsyncRead + AsyncWrite + Unpin> Receiver<T> {
    /// Receive the next websocket message.
    ///
    /// Fragmented messages will be concatenated and returned as one block.
    pub async fn receive(&mut self) -> Result<Incoming, Error> {
        let mut first_fragment_opcode = None;
        loop {
            if self.is_closed {
                log::debug!("can not receive, connection is closed");
                return Err(Error::Closed)
            }

            let mut header = self.receive_header().await?;
            log::trace!("recv: {}", header);

            // Handle control frames.
            if header.opcode().is_control() {
                self.read_buffer(&header).await?;
                let mut data = self.buffer.split_to(header.payload_len());
                base::Codec::apply_mask(&header, data.as_mut());
                if header.opcode() == OpCode::Pong {
                    return Ok(Incoming::Pong(Data::binary(data.into_bytes())))
                }
                self.on_control(&header, &mut Storage::Owned(data.into_bytes())).await?;
                continue
            }

            // Check if total message does not exceed maximum.
            if header.payload_len() + self.message.len() > self.max_message_size {
                log::warn!("accumulated message length exceeds maximum");
                return Err(Error::MessageTooLarge {
                    current: self.message.len() + header.payload_len(),
                    maximum: self.max_message_size
                })
            }

            self.read_buffer(&header).await?;
            base::Codec::apply_mask(&header, &mut self.buffer.as_mut()[.. header.payload_len()]);
            self.message.unsplit(self.buffer.split_to(header.payload_len()).into_bytes());

            match (header.is_fin(), header.opcode()) {
                (false, OpCode::Continue) => { // Intermediate message fragment.
                    if first_fragment_opcode.is_none() {
                        log::debug!("continue frame while not processing message fragments");
                        return Err(Error::UnexpectedOpCode(OpCode::Continue))
                    }
                    continue
                }
                (false, oc) => { // Initial message fragment.
                    if first_fragment_opcode.is_some() {
                        log::debug!("initial fragment while processing a fragmented message");
                        return Err(Error::UnexpectedOpCode(oc))
                    }
                    first_fragment_opcode = Some(oc);
                    self.decode_with_extensions(&mut header).await?;
                    continue
                }
                (true, OpCode::Continue) => { // Last message fragment.
                    if let Some(oc) = first_fragment_opcode.take() {
                        header.set_payload_len(self.message.len());
                        log::trace!("last fragement: total length = {} bytes", self.message.len());
                        self.decode_with_extensions(&mut header).await?;
                        header.set_opcode(oc);
                    } else {
                        log::debug!("last continue frame while not processing message fragments");
                        return Err(Error::UnexpectedOpCode(OpCode::Continue))
                    }
                }
                (true, oc) => { // Regular non-fragmented message.
                    if first_fragment_opcode.is_some() {
                        log::debug!("regular message while processing fragmented message");
                        return Err(Error::UnexpectedOpCode(oc))
                    }
                    self.decode_with_extensions(&mut header).await?
                }
            }

            if header.opcode() == OpCode::Text {
                return Ok(Incoming::Data(Data::text(crate::take(&mut self.message))))
            }

            return Ok(Incoming::Data(Data::binary(crate::take(&mut self.message))))
        }
    }

    /// Receive the next websocket message, skipping over control frames.
    ///
    /// Fragmented messages will be concatenated and returned as one block.
    pub async fn receive_data(&mut self) -> Result<Data, Error> {
        loop {
            if let Incoming::Data(d) = self.receive().await? {
                return Ok(d)
            }
        }
    }

    /// Read the next frame header.
    async fn receive_header(&mut self) -> Result<Header, Error> {
        if self.buffer.len() < MAX_HEADER_SIZE && self.buffer.remaining_mut() < MAX_HEADER_SIZE {
            // We may not have enough data in our read buffer and there may
            // not be enough capacity to read the next header, so reserve
            // some extra space to make sure we can read as much.
            const_assert!(MAX_HEADER_SIZE < BLOCK_SIZE);
            self.buffer.reserve(BLOCK_SIZE)
        }
        loop {
            match self.codec.decode_header(self.buffer.as_ref())? {
                Parsing::Done { value: header, offset } => {
                    debug_assert!(offset <= MAX_HEADER_SIZE);
                    self.buffer.split_to(offset);
                    return Ok(header)
                }
                Parsing::NeedMore(_) => self.buffer.read_from(&mut self.reader).await?
            }
        }
    }

    /// Read more data into read buffer if necessary.
    async fn read_buffer(&mut self, header: &Header) -> Result<(), Error> {
        if header.payload_len() <= self.buffer.len() {
            return Ok(()) // We have enough data in our buffer.
        }

        // We need to read data from the socket.
        // Ensure we have enough capacity and ask for at least a single block
        // so that we read a substantial amount.
        let add = std::cmp::max(BLOCK_SIZE, header.payload_len() - self.buffer.len());
        self.buffer.reserve(add);

        while self.buffer.len() < header.payload_len() {
            self.buffer.read_from(&mut self.reader).await?
        }

        Ok(())
    }

    /// Answer incoming control frames.
    async fn on_control(&mut self, header: &Header, data: &mut Storage<'_>) -> Result<(), Error> {
        debug_assert_eq!(data.as_ref().len(), header.payload_len());
        match header.opcode() {
            OpCode::Ping => {
                let mut answer = Header::new(OpCode::Pong);
                self.write(&mut answer, data).await?;
                self.flush().await?;
                Ok(())
            }
            OpCode::Pong => Ok(()),
            OpCode::Close => {
                self.is_closed = true;
                let (mut header, code) = close_answer(data.as_ref())?;
                if let Some(c) = code {
                    let data = c.to_be_bytes();
                    self.write(&mut header, &mut Storage::Shared(&data[..])).await?
                } else {
                    self.write(&mut header, &mut Storage::Shared(&[])).await?
                }
                self.flush().await?;
                self.writer.lock().await.close().await.or(Err(Error::Closed))
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
    async fn decode_with_extensions(&mut self, header: &mut Header) -> Result<(), Error> {
        if !self.has_extensions {
            return Ok(())
        }
        for e in self.extensions.lock().await.iter_mut() {
            log::trace!("decoding with extension: {}", e.name());
            e.decode(header, &mut self.message).map_err(Error::Extension)?
        }
        Ok(())
    }

    /// Flush the socket buffer.
    async fn flush(&mut self) -> Result<(), Error> {
        log::trace!("flushing connection");
        if self.is_closed {
            return Ok(())
        }
        self.writer.lock().await.flush().await.or(Err(Error::Closed))
    }

    /// Write final header and payload data to socket.
    ///
    /// The data will be masked if necessary.
    /// No extensions will be applied to header and payload data.
    async fn write(&mut self, header: &mut Header, data: &mut Storage<'_>) -> Result<(), Error> {
        write(self.mode, &mut self.codec, &mut self.writer, header, data, &mut self.mask_buffer).await
    }
}

impl<T: AsyncRead + AsyncWrite + Unpin> Sender<T> {
    /// Send a text value over the websocket connection.
    pub async fn send_text(&mut self, data: impl AsRef<str>) -> Result<(), Error> {
        let mut header = Header::new(OpCode::Text);
        self.send_frame(&mut header, &mut Storage::Shared(data.as_ref().as_bytes())).await
    }

    /// Send some binary data over the websocket connection.
    pub async fn send_binary(&mut self, data: impl AsRef<[u8]>) -> Result<(), Error> {
        let mut header = Header::new(OpCode::Binary);
        self.send_frame(&mut header, &mut Storage::Shared(data.as_ref())).await
    }

    /// Send some binary data over the websocket connection.
    ///
    /// In contrast to [`Sender::send_binary`] the provided data is modified
    /// in-place, e.g. if masking is necessary.
    pub async fn send_binary_mut(&mut self, mut data: impl AsMut<[u8]>) -> Result<(), Error> {
        let mut header = Header::new(OpCode::Binary);
        self.send_frame(&mut header, &mut Storage::Unique(data.as_mut())).await
    }

    /// Ping the remote end.
    pub async fn send_ping(&mut self, data: ByteSlice125<'_>) -> Result<(), Error> {
        let mut header = Header::new(OpCode::Ping);
        self.write(&mut header, &mut Storage::Shared(data.as_ref())).await
    }

    /// Send an unsolicited Pong to the remote.
    pub async fn send_pong(&mut self, data: ByteSlice125<'_>) -> Result<(), Error> {
        let mut header = Header::new(OpCode::Pong);
        self.write(&mut header, &mut Storage::Shared(data.as_ref())).await
    }

    /// Flush the socket buffer.
    pub async fn flush(&mut self) -> Result<(), Error> {
        log::trace!("flushing connection");
        self.writer.lock().await.flush().await.or(Err(Error::Closed))
    }

    /// Send a close message and close the connection.
    pub async fn close(&mut self) -> Result<(), Error> {
        log::trace!("closing connection");
        let mut header = Header::new(OpCode::Close);
        let code = 1000_u16.to_be_bytes(); // 1000 = normal closure
        self.write(&mut header, &mut Storage::Shared(&code[..])).await?;
        self.flush().await?;
        self.writer.lock().await.close().await.or(Err(Error::Closed))
    }

    /// Send arbitrary websocket frames.
    ///
    /// Before sending, extensions will be applied to header and payload data.
    async fn send_frame(&mut self, header: &mut Header, data: &mut Storage<'_>) -> Result<(), Error> {
        if !self.has_extensions {
            return self.write(header, data).await
        }

        for e in self.extensions.lock().await.iter_mut() {
            log::trace!("encoding with extension: {}", e.name());
            e.encode(header, data).map_err(Error::Extension)?
        }

        self.write(header, data).await
    }

    /// Write final header and payload data to socket.
    ///
    /// The data will be masked if necessary.
    /// No extensions will be applied to header and payload data.
    async fn write(&mut self, header: &mut Header, data: &mut Storage<'_>) -> Result<(), Error> {
        write(self.mode, &mut self.codec, &mut self.writer, header, data, &mut self.buffer).await
    }
}

/// Write header and payload data to socket.
async fn write<T: AsyncWrite + Unpin>
    ( mode: Mode
    , codec: &mut base::Codec
    , writer: &mut BiLock<BufWriter<WriteHalf<T>>>
    , header: &mut Header
    , data: &mut Storage<'_>
    , mask_buffer: &mut Vec<u8>
    ) -> Result<(), Error>
{
    if mode.is_client() {
        header.set_masked(true);
        header.set_mask(rand::random());
    }
    header.set_payload_len(data.as_ref().len());

    log::trace!("send: {}", header);

    let header_bytes = codec.encode_header(&header);
    let mut w = writer.lock().await;
    w.write_all(&header_bytes).await.or(Err(Error::Closed))?;

    if !header.is_masked() {
        return w.write_all(data.as_ref()).await.or(Err(Error::Closed))
    }

    match data {
        Storage::Shared(slice) => {
            mask_buffer.clear();
            mask_buffer.extend_from_slice(slice);
            base::Codec::apply_mask(header, mask_buffer);
            w.write_all(mask_buffer).await.or(Err(Error::Closed))
        }
        Storage::Unique(slice) => {
            base::Codec::apply_mask(header, slice);
            w.write_all(slice).await.or(Err(Error::Closed))
        }
        Storage::Owned(ref mut bytes) => {
            base::Codec::apply_mask(header, bytes);
            w.write_all(bytes).await.or(Err(Error::Closed))
        }
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

/// Turn a [`Receiver`] into a [`futures::Stream`].
pub fn into_stream<T>(r: Receiver<T>) -> impl stream::Stream<Item = Result<Incoming, Error>>
where
    T: AsyncRead + AsyncWrite + Unpin
{
    stream::unfold(r, |mut r| async {
        match r.receive().await {
            Ok(item) => Some((Ok(item), r)),
            Err(Error::Closed) => None,
            Err(e) => Some((Err(e), r))
        }
    })
}

/// Errors which may occur when sending or receiving messages.
#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// An I/O error was encountered.
    #[error("i/o error: {0}")]
    Io(#[source] io::Error),

    /// The base codec errored.
    #[error("codec error: {0}")]
    Codec(#[from] base::Error),

    /// An extension produced an error while encoding or decoding.
    #[error("extension error: {0}")]
    Extension(#[source] crate::BoxedError),

    /// An unexpected opcode was encountered.
    #[error("unexpected opcode: {0}")]
    UnexpectedOpCode(OpCode),

    /// A close reason was not correctly UTF-8 encoded.
    #[error("utf-8 opcode: {0}")]
    Utf8(#[from] std::str::Utf8Error),

    /// The total message payload data size exceeds the configured maximum.
    #[error("message too large: len >= {current}, maximum = {maximum}")]
    MessageTooLarge { current: usize, maximum: usize },

    /// The connection is closed.
    #[error("connection closed")]
    Closed
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        if e.kind() == io::ErrorKind::UnexpectedEof {
            Error::Closed
        } else {
            Error::Io(e)
        }
    }
}

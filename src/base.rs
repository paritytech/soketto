// Copyright (c) 2019 Parity Technologies (UK) Ltd.
// Copyright (c) 2016 twist developers
//
// Licensed under the Apache License, Version 2.0
// <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0> or the MIT
// license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. All files in the project carrying such notice may not be copied,
// modified, or distributed except according to those terms.

// This file is largely based on the original twist implementation.
// See [frame/base.rs] and [codec/base.rs].
//
// [frame/base.rs]: https://github.com/rustyhorde/twist/blob/449d8b75c2b3c5ca71db2253b0c1a00a818ac3bd/src/frame/base.rs
// [codec/base.rs]: https://github.com/rustyhorde/twist/blob/449d8b75c2b3c5ca71db2253b0c1a00a818ac3bd/src/codec/base.rs

//! A websocket [base] frame and accompanying tokio codec.
//!
//! [base]: https://tools.ietf.org/html/rfc6455#section-5.2

use bytes::{BufMut, Buf, BytesMut};
use std::{convert::TryFrom, fmt, io::{self, Cursor}};
use tokio_codec::{Decoder, Encoder};

// OpCode /////////////////////////////////////////////////////////////////////////////////////////

/// Operation codes defined in [RFC6455](https://tools.ietf.org/html/rfc6455#section-5.2).
#[derive(Debug, Eq, PartialEq, Clone, Copy)]
pub enum OpCode {
    /// A continuation frame of a fragmented message.
    Continue,
    /// A text data frame.
    Text,
    /// A binary data frame.
    Binary,
    /// A close control frame.
    Close,
    /// A ping control frame.
    Ping,
    /// A pong control frame.
    Pong,
    /// A reserved op code.
    Reserved3,
    /// A reserved op code.
    Reserved4,
    /// A reserved op code.
    Reserved5,
    /// A reserved op code.
    Reserved6,
    /// A reserved op code.
    Reserved7,
    /// A reserved op code.
    Reserved11,
    /// A reserved op code.
    Reserved12,
    /// A reserved op code.
    Reserved13,
    /// A reserved op code.
    Reserved14,
    /// A reserved op code.
    Reserved15
}

impl OpCode {
    /// Is this a control opcode?
    pub fn is_control(self) -> bool {
        if let OpCode::Close | OpCode::Ping | OpCode::Pong = self {
            true
        } else {
            false
        }
    }

    /// Is this opcode reserved?
    pub fn is_reserved(self) -> bool {
        match self {
            OpCode::Reserved3
            | OpCode::Reserved4
            | OpCode::Reserved5
            | OpCode::Reserved6
            | OpCode::Reserved7
            | OpCode::Reserved11
            | OpCode::Reserved12
            | OpCode::Reserved13
            | OpCode::Reserved14
            | OpCode::Reserved15 => true,
            _ => false
        }
    }
}

impl fmt::Display for OpCode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            OpCode::Continue => f.write_str("Continue"),
            OpCode::Text => f.write_str("Text"),
            OpCode::Binary => f.write_str("Binary"),
            OpCode::Close => f.write_str("Close"),
            OpCode::Ping => f.write_str("Ping"),
            OpCode::Pong => f.write_str("Pong"),
            OpCode::Reserved3 => f.write_str("Reserved:3"),
            OpCode::Reserved4 => f.write_str("Reserved:4"),
            OpCode::Reserved5 => f.write_str("Reserved:5"),
            OpCode::Reserved6 => f.write_str("Reserved:6"),
            OpCode::Reserved7 => f.write_str("Reserved:7"),
            OpCode::Reserved11 => f.write_str("Reserved:11"),
            OpCode::Reserved12 => f.write_str("Reserved:12"),
            OpCode::Reserved13 => f.write_str("Reserved:13"),
            OpCode::Reserved14 => f.write_str("Reserved:14"),
            OpCode::Reserved15 => f.write_str("Reserved:15")
        }
    }
}

/// Error returned by `OpCode::try_from` if an unknown opcode
/// number is encountered.
#[derive(Debug)]
pub struct UnknownOpCode(());

impl fmt::Display for UnknownOpCode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("unknown opcode")
    }
}

impl std::error::Error for UnknownOpCode {}

impl TryFrom<u8> for OpCode {
    type Error = UnknownOpCode;

    fn try_from(val: u8) -> Result<OpCode, Self::Error> {
        match val {
            0 => Ok(OpCode::Continue),
            1 => Ok(OpCode::Text),
            2 => Ok(OpCode::Binary),
            3 => Ok(OpCode::Reserved3),
            4 => Ok(OpCode::Reserved4),
            5 => Ok(OpCode::Reserved5),
            6 => Ok(OpCode::Reserved6),
            7 => Ok(OpCode::Reserved7),
            8 => Ok(OpCode::Close),
            9 => Ok(OpCode::Ping),
            10 => Ok(OpCode::Pong),
            11 => Ok(OpCode::Reserved11),
            12 => Ok(OpCode::Reserved12),
            13 => Ok(OpCode::Reserved13),
            14 => Ok(OpCode::Reserved14),
            15 => Ok(OpCode::Reserved15),
            _ => Err(UnknownOpCode(()))
        }
    }
}

impl From<OpCode> for u8 {
    fn from(opcode: OpCode) -> u8 {
        match opcode {
            OpCode::Continue => 0,
            OpCode::Text => 1,
            OpCode::Binary => 2,
            OpCode::Close => 8,
            OpCode::Ping => 9,
            OpCode::Pong => 10,
            OpCode::Reserved3 => 3,
            OpCode::Reserved4 => 4,
            OpCode::Reserved5 => 5,
            OpCode::Reserved6 => 6,
            OpCode::Reserved7 => 7,
            OpCode::Reserved11 => 11,
            OpCode::Reserved12 => 12,
            OpCode::Reserved13 => 13,
            OpCode::Reserved14 => 14,
            OpCode::Reserved15 => 15
        }
    }
}

// Data //////////////////////////////////////////////////////////////////////////////////////////

/// Application data of a websocket frame.
#[derive(Debug, Clone)]
pub enum Data {
    /// Application data of type binary (opcode 2)
    Binary(BytesMut),
    /// Application data of type text (opcode 1)
    Text(BytesMut)
}

impl Data {
    pub fn into_bytes(self) -> BytesMut {
        match self {
            Data::Binary(bytes) => bytes,
            Data::Text(bytes) => bytes,
        }
    }

    /// Get a unique reference to the underlying bytes.
    pub fn bytes_mut(&mut self) -> &mut BytesMut {
        match self {
            Data::Binary(bytes) => bytes,
            Data::Text(bytes) => bytes,
        }
    }

    pub fn is_text(&self) -> bool {
        if let Data::Text(_) = self { true } else { false }
    }

    pub fn is_binary(&self) -> bool {
        !self.is_text()
    }
}

impl AsRef<[u8]> for Data {
    fn as_ref(&self) -> &[u8] {
        match self {
            Data::Binary(bytes) => bytes,
            Data::Text(bytes) => bytes
        }
    }
}

impl AsMut<[u8]> for Data {
    fn as_mut(&mut self) -> &mut [u8] {
        match self {
            Data::Binary(bytes) => bytes,
            Data::Text(bytes) => bytes
        }
    }
}

// Frame //////////////////////////////////////////////////////////////////////////////////////////

/// A websocket [base](https://tools.ietf.org/html/rfc6455#section-5.2) frame.
#[derive(Debug, Clone)]
pub struct Frame {
    /// The `fin` flag.
    fin: bool,
    /// The `rsv1` flag.
    rsv1: bool,
    /// The `rsv2` flag.
    rsv2: bool,
    /// The `rsv3` flag.
    rsv3: bool,
    /// The 'mask' flag.
    masked: bool,
    /// The `opcode`
    opcode: OpCode,
    /// The `mask`.
    mask: u32,
    /// The optional payload data.
    payload_data: Option<Data>
}

impl Frame {
    /// Create a new frame with a given [`OpCode`].
    pub fn new(oc: OpCode) -> Self {
        Frame {
            fin: true,
            rsv1: false,
            rsv2: false,
            rsv3: false,
            masked: false,
            opcode: oc,
            mask: 0,
            payload_data: None
        }
    }

    /// Is the `fin` flag set?
    pub fn is_fin(&self) -> bool {
        self.fin
    }

    /// Set the `fin` flag.
    pub fn set_fin(&mut self, fin: bool) -> &mut Self {
        self.fin = fin;
        self
    }

    /// Is the `rsv1` flag set?
    pub fn is_rsv1(&self) -> bool {
        self.rsv1
    }

    /// Set the `rsv1` flag.
    pub fn set_rsv1(&mut self, rsv1: bool) -> &mut Self {
        self.rsv1 = rsv1;
        self
    }

    /// Is the `rsv2` flag set?
    pub fn is_rsv2(&self) -> bool {
        self.rsv2
    }

    /// Set the `rsv2` flag.
    pub fn set_rsv2(&mut self, rsv2: bool) -> &mut Self {
        self.rsv2 = rsv2;
        self
    }

    /// Is the `rsv3` flag set?
    pub fn is_rsv3(&self) -> bool {
        self.rsv3
    }

    /// Set the `rsv3` flag.
    pub fn set_rsv3(&mut self, rsv3: bool) -> &mut Self {
        self.rsv3 = rsv3;
        self
    }

    /// Is the `masked` flag set?
    pub fn is_masked(&self) -> bool {
        self.masked
    }

    /// Set the `masked` flag.
    pub fn set_masked(&mut self, masked: bool) -> &mut Self {
        self.masked = masked;
        self
    }

    /// Get the `opcode`.
    pub fn opcode(&self) -> OpCode {
        self.opcode
    }

    /// Set the `opcode`
    pub fn set_opcode(&mut self, opcode: OpCode) -> &mut Self {
        self.opcode = opcode;
        self
    }

    /// Get the `mask`.
    pub fn mask(&self) -> u32 {
        self.mask
    }

    /// Set the `mask`
    pub fn set_mask(&mut self, mask: u32) -> &mut Self {
        self.mask = mask;
        self
    }

    /// Get a reference to the payload data.
    pub fn payload_data(&self) -> Option<&Data> {
        self.payload_data.as_ref()
    }

    /// Get a mutable reference to the payload data.
    pub fn payload_data_mut(&mut self) -> Option<&mut Data> {
        self.payload_data.as_mut()
    }

    /// Consume frame and return payload data only.
    pub fn into_payload_data(self) -> Option<Data> {
        self.payload_data
    }

    /// Set the payload data.
    pub fn set_payload_data(&mut self, data: Option<Data>) -> &mut Self {
        self.payload_data = data;
        self
    }
}

// Frame codec ////////////////////////////////////////////////////////////////////////////////////

/// If the payload length byte is 126, the following two bytes represent the
/// actual payload length.
const TWO_EXT: u8 = 126;

/// If the payload length byte is 127, the following eight bytes represent
/// the actual payload length.
const EIGHT_EXT: u8 = 127;

/// Codec for encoding/decoding websocket [base] [`Frame`]s.
///
/// [base]: https://tools.ietf.org/html/rfc6455#section-5.2
#[derive(Debug)]
pub struct Codec {
    /// Decode state
    state: Option<DecodeState>,
    /// Maximum size of payload data per frame.
    max_data_size: u64,
    /// Bits reserved by an extension.
    reserved_bits: u8,
    /// OpCodes reserved by an extension.
    reserved_opcodes: u8
}

#[derive(Debug)]
enum DecodeState {
    /// Initial decoding state.
    Start,
    /// The first 2 bytes of a new frame have been decoded.
    /// Next is to decode the total frame length.
    FrameStart {
        frame: Frame,
        length_code: u8
    },
    /// The frame length has been decoded.
    /// Next is to read the masking key if present.
    FrameLength {
        frame: Frame,
        length: u64
    },
    /// The frame length and masking key have been decoded.
    /// As the final step, the payload data will be decoded.
    Body {
        frame: Frame,
        length: u64,
    }
}

impl Default for Codec {
    fn default() -> Self {
        Codec {
            state: Some(DecodeState::Start),
            max_data_size: 256 * 1024 * 1024,
            reserved_bits: 0,
            reserved_opcodes: 0
        }
    }
}

impl Codec {
    /// Create a new base frame codec.
    ///
    /// The codec will support decoding payload lengths up to 256 MiB
    /// (use `set_max_data_size` to change this value).
    pub fn new() -> Self {
        Codec::default()
    }

    /// Get the configured maximum payload length.
    pub fn max_data_size(&self) -> u64 {
        self.max_data_size
    }

    /// Limit the maximum size of payload data to `size` bytes.
    pub fn set_max_data_size(&mut self, size: u64) -> &mut Self {
        self.max_data_size = size;
        self
    }

    /// Is the given reserved opcode configured?
    pub fn has_reserved_opcode(&self, code: OpCode) -> bool {
        if !code.is_reserved() {
            return false
        }
        self.reserved_opcodes & u8::from(code) != 0
    }

    /// Add the reserved opcode.
    pub fn add_reserved_opcode(&mut self, code: OpCode) -> &mut Self {
        assert!(code.is_reserved());
        self.reserved_opcodes |= u8::from(code);
        self
    }

    /// Reset reserved opcodes.
    pub fn clear_reserved_opcodes(&mut self) {
        self.reserved_opcodes = 0
    }

    /// The reserved bits currently configured.
    pub fn reserved_bits(&self) -> (bool, bool, bool) {
        let r = self.reserved_bits;
        (r & 4 == 1, r & 2 == 1, r & 1 == 1)
    }

    /// Add to the reserved bits in use.
    pub fn add_reserved_bits(&mut self, bits: (bool, bool, bool)) -> &mut Self {
        let (r1, r2, r3) = bits;
        self.reserved_bits |= (r1 as u8) << 2 | (r2 as u8) << 1 | r3 as u8;
        self
    }

    /// Reset the reserved bits.
    pub fn clear_reserved_bits(&mut self) {
        self.reserved_bits = 0
    }
}

// Apply the unmasking to the payload data.
fn apply_mask(buf: &mut [u8], mask: u32) {
    let mask_buf = mask.to_be_bytes();
    for (byte, &key) in buf.iter_mut().zip(mask_buf.iter().cycle()) {
        *byte ^= key;
    }
}

impl Decoder for Codec {
    type Item = Frame;
    type Error = Error;

    fn decode(&mut self, buf: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        loop {
            match self.state.take() {
                Some(DecodeState::Start) => {
                    if buf.len() < 2 {
                        self.state = Some(DecodeState::Start);
                        return Ok(None)
                    }

                    let header_bytes = buf.split_to(2);
                    let first = header_bytes[0];
                    let second = header_bytes[1];

                    let fin = first & 0x80 != 0;
                    let opcode = OpCode::try_from(first & 0xF)?;
                    if opcode.is_reserved() && !self.has_reserved_opcode(opcode) {
                        return Err(Error::ReservedOpCode)
                    }
                    if opcode.is_control() && !fin {
                        return Err(Error::FragmentedControl)
                    }

                    let mut frame = Frame::new(opcode);
                    frame.set_fin(fin);

                    let rsv1 = first & 0x40 != 0;
                    if rsv1 && (self.reserved_bits & 4 == 0) {
                        return Err(Error::InvalidReservedBit(1))
                    }
                    frame.set_rsv1(rsv1);

                    let rsv2 = first & 0x20 != 0;
                    if rsv2 && (self.reserved_bits & 2 == 0) {
                        return Err(Error::InvalidReservedBit(2))
                    }
                    frame.set_rsv2(rsv2);

                    let rsv3 = first & 0x10 != 0;
                    if rsv3 && (self.reserved_bits & 1 == 0) {
                        return Err(Error::InvalidReservedBit(3))
                    }
                    frame.set_rsv3(rsv3);
                    frame.set_masked(second & 0x80 != 0);

                    self.state = Some(DecodeState::FrameStart { frame, length_code: second & 0x7F })
                }
                Some(DecodeState::FrameStart { frame, length_code }) => {
                    let len = match length_code {
                        TWO_EXT => {
                            if buf.len() < 2 {
                                self.state = Some(DecodeState::FrameStart { frame, length_code });
                                return Ok(None)
                            }
                            let len = u16::from_be_bytes([buf[0], buf[1]]);
                            buf.split_to(2);
                            u64::from(len)
                        }
                        EIGHT_EXT => {
                            if buf.len() < 8 {
                                self.state = Some(DecodeState::FrameStart { frame, length_code });
                                return Ok(None)
                            }
                            Cursor::new(buf.split_to(8)).get_u64_be()
                        }
                        n => u64::from(n)
                    };

                    if len > 125 && frame.opcode().is_control() {
                        return Err(Error::InvalidControlFrameLen)
                    }

                    if len > self.max_data_size {
                        return Err(Error::PayloadTooLarge {
                            actual: len,
                            maximum: self.max_data_size
                        })
                    }

                    self.state = Some(DecodeState::FrameLength { frame, length: len })
                }
                Some(DecodeState::FrameLength { mut frame, length }) => {
                    if !frame.is_masked() {
                        self.state = Some(DecodeState::Body { frame, length });
                        continue
                    }
                    if buf.len() < 4 {
                        self.state = Some(DecodeState::FrameLength { frame, length });
                        return Ok(None)
                    }
                    frame.set_mask(u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]));
                    buf.split_to(4);
                    self.state = Some(DecodeState::Body { frame, length })
                }
                Some(DecodeState::Body { mut frame, length: 0, .. }) => {
                    self.state = Some(DecodeState::Start);
                    if frame.payload_data.is_none() {
                        match frame.opcode {
                            OpCode::Binary => {
                                let d = Data::Binary(BytesMut::new());
                                frame.set_payload_data(Some(d));
                            }
                            OpCode::Text => {
                                let d = Data::Text(BytesMut::new());
                                frame.set_payload_data(Some(d));
                            }
                            _ => ()
                        }
                    }
                    return Ok(Some(frame))
                }
                Some(DecodeState::Body { mut frame, length }) => {
                    if (buf.len() as u64) < length {
                        if (buf.capacity() as u64) < length {
                            buf.reserve(length as usize - buf.len())
                        }
                        self.state = Some(DecodeState::Body { frame, length });
                        return Ok(None)
                    }
                    frame.payload_data =
                        if let OpCode::Text = frame.opcode {
                            Some(Data::Text(buf.split_to(length as usize)))
                        } else {
                            Some(Data::Binary(buf.split_to(length as usize)))
                        };
                    if frame.is_masked() {
                        let mask = frame.mask();
                        if let Some(ref mut d) = frame.payload_data {
                            apply_mask(d.as_mut(), mask)
                        }
                    }
                    self.state = Some(DecodeState::Start);
                    return Ok(Some(frame))
                }
                None => return Err(Error::IllegalCodecState)
            }
        }
    }
}

impl Encoder for Codec {
    type Item = Frame;
    type Error = Error;

    fn encode(&mut self, frame: Self::Item, buf: &mut BytesMut) -> Result<(), Self::Error> {
        buf.reserve(2);

        let mut first_byte = 0_u8;
        if frame.is_fin() {
            first_byte |= 0x80
        }
        if frame.is_rsv1() {
            first_byte |= 0x40
        }
        if frame.is_rsv2() {
            first_byte |= 0x20
        }
        if frame.is_rsv3() {
            first_byte |= 0x10
        }

        let opcode: u8 = frame.opcode().into();
        first_byte |= opcode;

        buf.put(first_byte);

        let mut second_byte = 0_u8;

        if frame.is_masked() {
            second_byte |= 0x80
        }

        let len = frame.payload_data().map(|d| d.as_ref().len()).unwrap_or(0);

        if len < usize::from(TWO_EXT) {
            second_byte |= len as u8;
            buf.put(second_byte);
        } else if len <= usize::from(u16::max_value()) {
            second_byte |= TWO_EXT;
            buf.put(second_byte);
            buf.extend_from_slice(&(len as u16).to_be_bytes())
        } else {
            second_byte |= EIGHT_EXT;
            buf.put(second_byte);
            buf.extend_from_slice(&len.to_be_bytes())
        }

        if frame.is_masked() {
            buf.extend_from_slice(&frame.mask().to_be_bytes())
        }

        let mask = frame.mask();
        let is_masked = frame.is_masked();

        if let Some(mut data) = frame.into_payload_data() {
            if is_masked {
                apply_mask(data.as_mut(), mask)
            }
            buf.extend_from_slice(data.as_ref())
        }

        Ok(())
    }
}

// Codec error type ///////////////////////////////////////////////////////////////////////////////

#[derive(Debug)]
pub enum Error {
    /// An I/O error has been encountered.
    Io(io::Error),
    /// Some unknown opcode number has been decoded.
    UnknownOpCode,
    /// The opcode decoded is reserved.
    ReservedOpCode,
    /// A fragmented control frame (fin bit not set) has been decoded.
    FragmentedControl,
    /// A control frame with an invalid length code has been decoded.
    InvalidControlFrameLen,
    /// The reserved bit is invalid.
    InvalidReservedBit(u8),
    /// The payload length of a frame exceeded the configured maximum.
    PayloadTooLarge { actual: u64, maximum: u64 },
    /// The codec transitions into an illegal state.
    /// This happens if the codec is used after it has returned an error.
    IllegalCodecState,

    #[doc(hidden)]
    __Nonexhaustive
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::Io(e) => write!(f, "i/o error: {}", e),
            Error::UnknownOpCode => f.write_str("unknown opcode"),
            Error::ReservedOpCode => f.write_str("reserved opcode"),
            Error::FragmentedControl => f.write_str("fragmented control frame"),
            Error::IllegalCodecState => f.write_str("illegal codec state"),
            Error::InvalidControlFrameLen => f.write_str("invalid control frame length"),
            Error::InvalidReservedBit(i) => write!(f, "invalid reserved bit: {}", i),
            Error::PayloadTooLarge { actual, maximum } =>
                write!(f, "payload to large: len = {}, maximum = {}", actual, maximum),
            Error::__Nonexhaustive => f.write_str("__Nonexhaustive")
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io(e) => Some(e),
            Error::UnknownOpCode
            | Error::ReservedOpCode
            | Error::FragmentedControl
            | Error::InvalidControlFrameLen
            | Error::InvalidReservedBit(_)
            | Error::IllegalCodecState
            | Error::PayloadTooLarge {..}
            | Error::__Nonexhaustive => None
        }
    }
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Error::Io(e)
    }
}

impl From<UnknownOpCode> for Error {
    fn from(_: UnknownOpCode) -> Self {
        Error::UnknownOpCode
    }
}

// Tests //////////////////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod test {
    use super::{Frame, OpCode, Codec};
    use bytes::BytesMut;
    use tokio_codec::Decoder;

    // Payload on control frame must be 125 bytes or less. 2nd byte must be 0xFD or less.
    const CTRL_PAYLOAD_LEN : [u8; 9] = [0x89, 0xFE, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];

    // Truncated Frames, should return Ok(None)
    // One byte of the 2 byte header is ok.
    const PARTIAL_HEADER: [u8; 1] = [0x89];
    // Between 0 and 2 bytes of a 2 byte length block is ok.
    const PARTIAL_LENGTH_1: [u8; 3] = [0x89, 0xFE, 0x01];
    // Between 0 and 8 bytes of an 8 byte length block is ok.
    const PARTIAL_LENGTH_2: [u8; 6] = [0x89, 0xFF, 0x01, 0x02, 0x03, 0x04];
    // Between 0 and 4 bytes of the 4 byte mask is ok.
    const PARTIAL_MASK: [u8; 6] = [0x82, 0xFE, 0x01, 0x02, 0x00, 0x00];
    // Between 0 and X bytes of the X byte payload is ok.
    const PARTIAL_PAYLOAD: [u8; 8] = [0x82, 0x85, 0x01, 0x02, 0x03, 0x04, 0x00, 0x00];

    // Good Frames, should return Ok(Some(x))
    const PING_NO_DATA: [u8; 6] = [0x89, 0x80, 0x00, 0x00, 0x00, 0x01];

    fn decode(buf: &[u8]) -> Result<Option<Frame>, super::Error> {
        let mut eb = BytesMut::with_capacity(256);
        eb.extend(buf);
        let mut fc = Codec::new();
        fc.decode(&mut eb)
    }

    #[test]
    /// Checking that partial header returns Ok(None).
    fn decode_partial_header() {
        if let Ok(None) = decode(&PARTIAL_HEADER) {
            assert!(true);
        } else {
            assert!(false);
        }
    }

    #[test]
    /// Checking that partial 2 byte length returns Ok(None).
    fn decode_partial_len_1() {
        if let Ok(None) = decode(&PARTIAL_LENGTH_1) {
            assert!(true);
        } else {
            assert!(false);
        }
    }

    #[test]
    /// Checking that partial 8 byte length returns Ok(None).
    fn decode_partial_len_2() {
        if let Ok(None) = decode(&PARTIAL_LENGTH_2) {
            assert!(true);
        } else {
            assert!(false);
        }
    }

    #[test]
    /// Checking that partial mask returns Ok(None).
    fn decode_partial_mask() {
        if let Ok(None) = decode(&PARTIAL_MASK) {
            assert!(true);
        } else {
            assert!(false);
        }
    }

    #[test]
    /// Checking that partial payload returns Ok(None).
    fn decode_partial_payload() {
        if let Ok(None) = decode(&PARTIAL_PAYLOAD) {
            assert!(true);
        } else {
            assert!(false);
        }
    }

    #[test]
    /// Checking that partial mask returns Ok(None).
    fn decode_invalid_control_payload_len() {
        if let Err(_e) = decode(&CTRL_PAYLOAD_LEN) {
            assert!(true);
        } else {
            assert!(false);
        }
    }

    #[test]
    /// Checking that rsv1, rsv2, and rsv3 bit set returns error.
    fn decode_reserved() {
        // rsv1, rsv2, and rsv3.
        let reserved = [0x90, 0xa0, 0xc0];

        for res in &reserved {
            let mut buf = Vec::with_capacity(2);
            let mut first_byte = 0_u8;
            first_byte |= *res;
            buf.push(first_byte);
            buf.push(0x00);
            if let Err(_e) = decode(&buf) {
                assert!(true);
                // TODO: Assert error type when implemented.
            } else {
                eprintln!("rsv should not be set: {}", res);
                assert!(false);
            }
        }
    }

    #[test]
    /// Checking that a control frame, where fin bit is 0, returns an error.
    fn decode_fragmented_control() {
        let second_bytes = [8, 9, 10];

        for sb in &second_bytes {
            let mut buf = Vec::with_capacity(2);
            let mut first_byte = 0_u8;
            first_byte |= *sb;
            buf.push(first_byte);
            buf.push(0x00);
            if let Err(_e) = decode(&buf) {
                assert!(true);
                // TODO: Assert error type when implemented.
            } else {
                eprintln!("control frame is marked as fragment");
                assert!(false);
            }
        }
    }

    #[test]
    /// Checking that reserved opcodes return an error.
    fn decode_reserved_opcodes() {
        let reserved = [3, 4, 5, 6, 7, 11, 12, 13, 14, 15];

        for res in &reserved {
            let mut buf = Vec::with_capacity(2);
            let mut first_byte = 0_u8;
            first_byte |= 0x80;
            first_byte |= *res;
            buf.push(first_byte);
            buf.push(0x00);
            if let Err(_e) = decode(&buf) {
                assert!(true);
                // TODO: Assert error type when implemented.
            } else {
                eprintln!("opcode {} should be reserved", res);
                assert!(false);
            }
        }
    }

    #[test]
    fn decode_ping_no_data() {
        if let Ok(Some(frame)) = decode(&PING_NO_DATA) {
            assert!(frame.is_fin());
            assert!(!frame.is_rsv1());
            assert!(!frame.is_rsv2());
            assert!(!frame.is_rsv3());
            assert!(frame.opcode() == OpCode::Ping);
            assert!(frame.payload_data().is_none())
        } else {
            assert!(false)
        }
    }
}

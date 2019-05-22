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
    /// Indicates a continuation frame of a fragmented message.
    Continue,
    /// Indicates a text data frame.
    Text,
    /// Indicates a binary data frame.
    Binary,
    /// Indicates a close control frame.
    Close,
    /// Indicates a ping control frame.
    Ping,
    /// Indicates a pong control frame.
    Pong,
    /// Indicates a reserved op code.
    Reserved
}

impl OpCode {
    /// Is this a control opcode?
    pub fn is_control(self) -> bool {
        match self {
            OpCode::Close | OpCode::Ping | OpCode::Pong => true,
            _ => false
        }
    }

    /// Is this opcode reserved?
    pub fn is_reserved(self) -> bool {
        match self {
            OpCode::Reserved => true,
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
            OpCode::Reserved => f.write_str("Reserved")
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
            8 => Ok(OpCode::Close),
            9 => Ok(OpCode::Ping),
            10 => Ok(OpCode::Pong),
            3 ... 7 | 11 ... 15 => Ok(OpCode::Reserved),
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
            OpCode::Reserved => 3
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
    /// The optional application data.
    application_data: Option<Data>
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
            application_data: None
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

    /// Get the application data.
    pub fn application_data(&self) -> &[u8] {
        self.application_data.as_ref().map(|d| d.as_ref()).unwrap_or(&[])
    }

    /// Consume frame and return application data only.
    pub fn into_application_data(self) -> Option<Data> {
        self.application_data
    }

    /// Set the application data.
    pub fn set_application_data(&mut self, data: Option<Data>) -> &mut Self {
        self.application_data = data;
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
    /// Bits reserved by extensions.
    reserved_bits: u8
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

impl Codec {
    pub fn new() -> Self {
        Self {
            state: Some(DecodeState::Start),
            reserved_bits: 0
        }
    }
}

// Apply the unmasking to the application data.
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
                    let opcode = OpCode::try_from(first & 0x0F)?;
                    if opcode.is_reserved() {
                        return Err(Error::ReservedOpCode)
                    }
                    if opcode.is_control() && !fin {
                        return Err(Error::FragmentedControl)
                    }

                    let mut frame = Frame::new(opcode);
                    frame.set_fin(fin);

                    let rsv1 = first & 0x40 != 0;
                    if rsv1 && (self.reserved_bits & 0x4 == 0) {
                        return Err(Error::InvalidReservedBit(1))
                    }
                    frame.set_rsv1(rsv1);

                    let rsv2 = first & 0x20 != 0;
                    if rsv2 && (self.reserved_bits & 0x2 == 0) {
                        return Err(Error::InvalidReservedBit(2))
                    }
                    frame.set_rsv2(rsv2);

                    let rsv3 = first & 0x10 != 0;
                    if rsv3 && (self.reserved_bits & 0x1 == 0) {
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
                    if frame.application_data.is_none() {
                        match frame.opcode {
                            OpCode::Binary => {
                                let d = Data::Binary(BytesMut::new());
                                frame.set_application_data(Some(d));
                            }
                            OpCode::Text => {
                                let d = Data::Text(BytesMut::new());
                                frame.set_application_data(Some(d));
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
                    frame.application_data =
                        if let OpCode::Text = frame.opcode {
                            Some(Data::Text(buf.split_to(length as usize)))
                        } else {
                            Some(Data::Binary(buf.split_to(length as usize)))
                        };
                    if frame.is_masked() {
                        let mask = frame.mask();
                        if let Some(ref mut d) = frame.application_data {
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

        let len = frame.application_data().len();

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

        if !frame.application_data().is_empty() {
            buf.extend_from_slice(frame.application_data())
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

    // Bad Frames, should err
    // Mask bit must be one. 2nd byte must be 0x80 or greater.
    const _NO_MASK: [u8; 2]          = [0x89, 0x00];
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
            assert!(frame.application_data().is_empty())
        } else {
            assert!(false)
        }
    }
}

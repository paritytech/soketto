//! Codec for encoding/decoding websocket [base] frames.
//!
//! [base]: https://tools.ietf.org/html/rfc6455#section-5.2
use bytes::{BufMut, Buf, BytesMut, BigEndian};
use encoding::{Encoding, DecoderTrap};
use encoding::all::UTF_8;
use frame::base::{Frame, OpCode};
use std::io::{self, Cursor};
use tokio_io::codec::{Decoder, Encoder};
use util;

/// If the payload length byte is 126, the following two bytes represent the actual payload
/// length.
const TWO_EXT: u8 = 126;
/// If the payload length byte is 127, the following eight bytes represent the actual payload
/// length.
const EIGHT_EXT: u8 = 127;

/// Indicates the state of the decoding process for this frame.
#[derive(Debug, Clone)]
pub enum DecodeState {
    /// None of the frame has been decoded.
    NONE,
    /// The header has been decoded.
    HEADER,
    /// The length has been decoded.
    LENGTH,
    /// The mask has been decoded.
    MASK,
    /// The decoding is complete.
    FULL,
}

impl Default for DecodeState {
    fn default() -> DecodeState {
        DecodeState::NONE
    }
}

/// Codec for encoding/decoding websocket [base] frames.
///
/// [base]: https://tools.ietf.org/html/rfc6455#section-5.2
#[derive(Clone, Debug, Default)]
pub struct FrameCodec {
    /// Is this a client frame?
    client: bool,
    /// The `fin` flag.
    fin: bool,
    /// The `rsv1` flag.
    rsv1: bool,
    /// The `rsv2` flag.
    rsv2: bool,
    /// The `rsv3` flag.
    rsv3: bool,
    /// The `opcode`
    opcode: OpCode,
    /// The `masked` flag
    masked: bool,
    /// The length code.
    length_code: u8,
    /// The `payload_length`
    payload_length: u64,
    /// The optional `mask`
    mask_key: u32,
    /// The optional `extension_data`
    extension_data: Option<Vec<u8>>,
    /// The optional `application_data`
    application_data: Vec<u8>,
    /// Decode state
    state: DecodeState,
    /// Minimum length required to parse the next part of the frame.
    min_len: u64,
    /// Bits reserved by extensions.
    reserved_bits: u8,
}

impl FrameCodec {
    /// Set the `client` flag.
    pub fn set_client(&mut self, client: bool) -> &mut FrameCodec {
        self.client = client;
        self
    }

    /// Set the bits reserved by extensions (0-8 are valid values).
    pub fn set_reserved_bits(&mut self, reserved_bits: u8) -> &mut FrameCodec {
        self.reserved_bits = reserved_bits;
        self
    }
}

/// Apply the unmasking to the application data.
fn apply_mask(buf: &mut [u8], mask: u32) -> Result<(), io::Error> {
    let mut mask_buf = BytesMut::with_capacity(4);
    mask_buf.put_u32::<BigEndian>(mask);
    let iter = buf.iter_mut().zip(mask_buf.iter().cycle());
    for (byte, &key) in iter {
        *byte ^= key;
    }
    Ok(())
}

impl Decoder for FrameCodec {
    type Item = Frame;
    type Error = io::Error;

    fn decode(&mut self, buf: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        let buf_len = buf.len();
        if buf_len == 0 {
            return Ok(None);
        }

        self.min_len = 0;
        loop {
            match self.state {
                DecodeState::NONE => {
                    self.min_len += 2;
                    // Split off the 2 'header' bytes.
                    if (buf_len as u64) < self.min_len {
                        return Ok(None);
                    }
                    let header_bytes = buf.split_to(2);
                    let header = &header_bytes;
                    let first = header[0];
                    let second = header[1];

                    // Extract the details
                    self.fin = first & 0x80 != 0;
                    self.rsv1 = first & 0x40 != 0;
                    if self.rsv1 && (self.reserved_bits & 0x4 == 0) {
                        return Err(util::other("invalid rsv1 bit set"));
                    }

                    self.rsv2 = first & 0x20 != 0;
                    if self.rsv2 && (self.reserved_bits & 0x2 == 0) {
                        return Err(util::other("invalid rsv2 bit set"));
                    }

                    self.rsv3 = first & 0x10 != 0;
                    if self.rsv3 && (self.reserved_bits & 0x1 == 0) {
                        return Err(util::other("invalid rsv3 bit set"));
                    }

                    self.opcode = OpCode::from((first & 0x0F) as u8);
                    if self.opcode.is_invalid() {
                        return Err(util::other("invalid opcode set"));
                    }
                    if self.opcode.is_control() && !self.fin {
                        return Err(util::other("control frames must not be fragmented"));
                    }

                    self.masked = second & 0x80 != 0;
                    if !self.masked && !self.client {
                        return Err(util::other("all client frames must have a mask"));
                    }

                    self.length_code = (second & 0x7F) as u8;
                    self.state = DecodeState::HEADER;
                }
                DecodeState::HEADER => {
                    if self.length_code == TWO_EXT {
                        self.min_len += 2;
                        if (buf_len as u64) < self.min_len {
                            self.min_len -= 2;
                            return Ok(None);
                        }
                        let len = Cursor::new(buf.split_to(2)).get_u16::<BigEndian>();
                        self.payload_length = len as u64;
                        self.state = DecodeState::LENGTH;
                    } else if self.length_code == EIGHT_EXT {
                        self.min_len += 8;
                        if (buf_len as u64) < self.min_len {
                            self.min_len -= 8;
                            return Ok(None);
                        }
                        let len = Cursor::new(buf.split_to(8)).get_u64::<BigEndian>();
                        self.payload_length = len as u64;
                        self.state = DecodeState::LENGTH;
                    } else {
                        self.payload_length = self.length_code as u64;
                        self.state = DecodeState::LENGTH;
                    }
                    if self.payload_length > 125 && self.opcode.is_control() {
                        return Err(util::other("invalid control frame"));
                    }
                }
                DecodeState::LENGTH => {
                    if self.masked {
                        self.min_len += 4;
                        if (buf_len as u64) < self.min_len {
                            self.min_len -= 4;
                            return Ok(None);
                        }
                        let mask = Cursor::new(buf.split_to(4)).get_u32::<BigEndian>();
                        self.mask_key = mask;
                        self.state = DecodeState::MASK;
                    } else {
                        self.mask_key = 0;
                        self.state = DecodeState::MASK;
                    }
                }
                DecodeState::MASK => {
                    if self.payload_length > 0 {
                        let mask = self.mask_key;
                        let app_data_len = self.application_data.len();
                        if buf.is_empty() {
                            return Ok(None);
                        } else if ((buf.len() + app_data_len) as u64) < self.payload_length {
                            self.application_data.extend(buf.take());
                            if self.opcode == OpCode::Text {
                                apply_mask(&mut self.application_data, mask)?;
                                match UTF_8.decode(&self.application_data, DecoderTrap::Strict) {
                                    Ok(_) => {}
                                    Err(e) => {
                                        if &e == "invalid sequence" {
                                            return Err(util::other(&e));
                                        }
                                    }
                                }
                                apply_mask(&mut self.application_data, mask)?;
                            }
                            return Ok(None);
                        } else {
                            #[cfg_attr(feature = "cargo-clippy", allow(cast_possible_truncation))]
                            let split_len = (self.payload_length as usize) - app_data_len;
                            self.application_data.extend(buf.split_to(split_len));
                            if self.masked {
                                apply_mask(&mut self.application_data, mask)?;
                            }
                            self.state = DecodeState::FULL;
                        }
                    } else {
                        self.state = DecodeState::FULL;
                    }
                }
                DecodeState::FULL => break,
            }
        }

        Ok(Some(self.clone().into()))
    }
}

impl Encoder for FrameCodec {
    type Item = Frame;
    type Error = io::Error;

    fn encode(&mut self, msg: Self::Item, buf: &mut BytesMut) -> io::Result<()> {
        let mut first_byte = 0_u8;

        if msg.fin() {
            first_byte |= 0x80;
        }

        if msg.rsv1() {
            first_byte |= 0x40;
        }

        if msg.rsv2() {
            first_byte |= 0x20;
        }

        if msg.rsv3() {
            first_byte |= 0x10;
        }

        let opcode: u8 = msg.opcode().into();
        first_byte |= opcode;
        buf.put(first_byte);

        let mut second_byte = 0_u8;

        if msg.masked() {
            second_byte |= 0x80;
        }

        let len = msg.payload_length();
        if len < TWO_EXT as u64 {
            #[cfg_attr(feature = "cargo-clippy", allow(cast_possible_truncation))]
            let cast_len = len as u8;
            second_byte |= cast_len;
            buf.put(second_byte);
        } else if len < 65536 {
            second_byte |= TWO_EXT;
            let mut len_buf = BytesMut::with_capacity(2);
            #[cfg_attr(feature = "cargo-clippy", allow(cast_possible_truncation))]
            let cast_len = len as u16;
            len_buf.put_u16::<BigEndian>(cast_len);
            buf.put(second_byte);
            buf.extend(len_buf);
        } else {
            second_byte |= EIGHT_EXT;
            let mut len_buf = BytesMut::with_capacity(8);
            len_buf.put_u64::<BigEndian>(len);
            buf.put(second_byte);
            buf.extend(len_buf);
        }

        if msg.masked() {
            let mut mask_buf = BytesMut::with_capacity(4);
            mask_buf.put_u32::<BigEndian>(msg.mask());
            buf.extend(mask_buf);
        }

        if !msg.application_data().is_empty() {
            buf.extend(msg.application_data().clone());
        }

        Ok(())
    }
}

impl From<FrameCodec> for Frame {
    fn from(frame_codec: FrameCodec) -> Frame {
        let mut frame: Frame = Default::default();
        frame.set_fin(frame_codec.fin);
        frame.set_rsv1(frame_codec.rsv1);
        frame.set_rsv2(frame_codec.rsv2);
        frame.set_rsv3(frame_codec.rsv3);
        frame.set_masked(frame_codec.masked);
        frame.set_opcode(frame_codec.opcode);
        frame.set_mask(frame_codec.mask_key);
        frame.set_payload_length(frame_codec.payload_length);
        frame.set_application_data(frame_codec.application_data);
        frame.set_extension_data(frame_codec.extension_data);
        frame
    }
}

#[cfg(test)]
mod test {
    use super::FrameCodec;
    use bytes::BytesMut;
    use frame::base::{Frame, OpCode};
    use std::io;
    use tokio_io::codec::Decoder;
    use util;

    // Bad Frames, should err
    #[cfg_attr(rustfmt, rustfmt_skip)]
    // Mask bit must be one. 2nd byte must be 0x80 or greater.
    const NO_MASK: [u8; 2]           = [0x89, 0x00];
    #[cfg_attr(rustfmt, rustfmt_skip)]
    // Payload on control frame must be 125 bytes or less. 2nd byte must be 0xFD or less.
    const CTRL_PAYLOAD_LEN : [u8; 9] = [0x89, 0xFE, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];

    // Truncated Frames, should return Ok(None)
    #[cfg_attr(rustfmt, rustfmt_skip)]
    // One byte of the 2 byte header is ok.
    const PARTIAL_HEADER: [u8; 1]    = [0x89];
    #[cfg_attr(rustfmt, rustfmt_skip)]
    // Between 0 and 2 bytes of a 2 byte length block is ok.
    const PARTIAL_LENGTH_1: [u8; 3]  = [0x89, 0xFE, 0x01];
    #[cfg_attr(rustfmt, rustfmt_skip)]
    // Between 0 and 8 bytes of an 8 byte length block is ok.
    const PARTIAL_LENGTH_2: [u8; 6]  = [0x89, 0xFF, 0x01, 0x02, 0x03, 0x04];
    #[cfg_attr(rustfmt, rustfmt_skip)]
    // Between 0 and 4 bytes of the 4 byte mask is ok.
    const PARTIAL_MASK: [u8; 6]      = [0x82, 0xFE, 0x01, 0x02, 0x00, 0x00];
    #[cfg_attr(rustfmt, rustfmt_skip)]
    // Between 0 and X bytes of the X byte payload is ok.
    const PARTIAL_PAYLOAD: [u8; 8]    = [0x82, 0x85, 0x01, 0x02, 0x03, 0x04, 0x00, 0x00];

    // Good Frames, should return Ok(Some(x))
    #[cfg_attr(rustfmt, rustfmt_skip)]
    const PING_NO_DATA: [u8; 6]     = [0x89, 0x80, 0x00, 0x00, 0x00, 0x01];

    fn decode(buf: &[u8]) -> Result<Option<Frame>, io::Error> {
        let mut eb = BytesMut::with_capacity(256);
        eb.extend(buf);
        let mut fc: FrameCodec = Default::default();
        fc.set_client(false);
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
                util::stdo(&format!("rsv should not be set: {}", res));
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
                util::stdo("control frame {} is marked as fragment");
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
                util::stdo(&format!("opcode {} should be reserved", res));
                assert!(false);
            }
        }
    }

    #[test]
    /// Checking that a decode frame (always from client) with the mask bit not set returns an
    /// error.
    fn decode_no_mask() {
        if let Err(_e) = decode(&NO_MASK) {
            assert!(true);
            // TODO: Assert error type when implemented.
        } else {
            util::stdo("decoded frames should always have a mask");
            assert!(false);
        }
    }

    #[test]
    fn decode_ping_no_data() {
        if let Ok(Some(frame)) = decode(&PING_NO_DATA) {
            assert!(frame.fin());
            assert!(!frame.rsv1());
            assert!(!frame.rsv2());
            assert!(!frame.rsv3());
            assert!(frame.opcode() == OpCode::Ping);
            assert!(frame.payload_length() == 0);
            assert!(frame.extension_data().is_none());
            assert!(frame.application_data().is_empty());
        } else {
            assert!(false);
        }
    }
}

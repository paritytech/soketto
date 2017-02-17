//! Codec for dedoding/encoding websocket base frames.
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use frame::base::{Frame, OpCode};
use std::io::{self, Cursor};
use tokio_core::io::{Codec, EasyBuf};
use util;

/// If the payload length byte is 126, the following two bytes represent the actual payload
/// length.
const TWO_EXT: u8 = 126;
/// If the payload length byte is 127, the following eight bytes represent the actual payload
/// length.
const EIGHT_EXT: u8 = 127;

#[derive(Debug, Clone)]
/// Indicates the state of the decoding process for this frame.
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

#[derive(Clone, Debug, Default)]
/// Codec for dedoding/encoding websocket base frames.
pub struct FrameCodec {
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
    mask_key: Option<u32>,
    /// The optional `extension_data`
    extension_data: Option<Vec<u8>>,
    /// The optional `application_data`
    application_data: Option<Vec<u8>>,
    /// Decode state
    state: DecodeState,
    /// Minimum length required to parse the next part of the frame.
    min_len: u64,
}

impl FrameCodec {
    /// Apply the unmasking to the application data.
    fn apply_mask(&mut self, buf: &mut [u8], mask: u32) -> Result<(), io::Error> {
        let mut mask_buf = Vec::with_capacity(4);
        try!(mask_buf.write_u32::<BigEndian>(mask));
        let iter = buf.iter_mut().zip(mask_buf.iter().cycle());
        for (byte, &key) in iter {
            *byte ^= key
        }
        Ok(())
    }
}

impl Codec for FrameCodec {
    type In = Frame;
    type Out = Frame;

    fn decode(&mut self, buf: &mut EasyBuf) -> Result<Option<Self::In>, io::Error> {
        let buf_len = buf.len();
        if buf_len == 0 {
            return Ok(None);
        }
        self.min_len = 0;
        loop {
            match self.state {
                DecodeState::NONE => {
                    self.min_len += 2;
                    // Split of the 2 'header' bytes.
                    #[cfg_attr(feature = "cargo-clippy", allow(cast_possible_truncation))]
                    let size = self.min_len as usize;
                    if buf_len < size {
                        return Ok(None);
                    }
                    let header_bytes = buf.drain_to(2);
                    let header = header_bytes.as_slice();
                    let first = header[0];
                    let second = header[1];

                    // Extract the details
                    self.fin = first & 0x80 != 0;
                    self.rsv1 = first & 0x40 != 0;
                    if self.rsv1 {
                        return Err(util::other("invalid rsv1 bit set"));
                    }
                    self.rsv2 = first & 0x20 != 0;
                    if self.rsv2 {
                        return Err(util::other("invlid rsv2 bit set"));
                    }
                    self.rsv3 = first & 0x10 != 0;
                    if self.rsv3 {
                        return Err(util::other("invlid rsv3 bit set"));
                    }
                    self.opcode = OpCode::from((first & 0x0F) as u8);
                    if self.opcode.is_invalid() {
                        return Err(util::other("invalid opcode set"));
                    }

                    if self.opcode.is_control() && !self.fin {
                        return Err(util::other("control frames must not be fragmented"));
                    }
                    self.masked = second & 0x80 != 0;
                    self.length_code = (second & 0x7F) as u8;
                    self.state = DecodeState::HEADER;
                }
                DecodeState::HEADER => {
                    if self.length_code == TWO_EXT {
                        self.min_len += 2;
                        #[cfg_attr(feature = "cargo-clippy", allow(cast_possible_truncation))]
                        let size = self.min_len as usize;
                        if buf_len < size {
                            self.min_len -= 2;
                            return Ok(None);
                        }
                        let mut rdr = Cursor::new(buf.drain_to(2));
                        if let Ok(len) = rdr.read_u16::<BigEndian>() {
                            self.payload_length = len as u64;
                            self.state = DecodeState::LENGTH;
                        } else {
                            return Err(util::other("invalid length bytes"));
                        }
                    } else if self.length_code == EIGHT_EXT {
                        self.min_len += 8;
                        #[cfg_attr(feature = "cargo-clippy", allow(cast_possible_truncation))]
                        let size = self.min_len as usize;
                        if buf_len < size {
                            self.min_len -= 8;
                            return Ok(None);
                        }
                        let mut rdr = Cursor::new(buf.drain_to(8));
                        if let Ok(len) = rdr.read_u64::<BigEndian>() {
                            self.payload_length = len as u64;
                            self.state = DecodeState::LENGTH;
                        } else {
                            return Err(util::other("invalid length bytes"));
                        }
                    } else {
                        self.payload_length = self.length_code as u64;
                        self.state = DecodeState::LENGTH;
                    }
                }
                DecodeState::LENGTH => {
                    if self.masked {
                        self.min_len += 4;
                        #[cfg_attr(feature = "cargo-clippy", allow(cast_possible_truncation))]
                        let size = self.min_len as usize;
                        if buf_len < size {
                            self.min_len -= 4;
                            return Ok(None);
                        }
                        let mut rdr = Cursor::new(buf.drain_to(4));
                        if let Ok(mask) = rdr.read_u32::<BigEndian>() {
                            self.mask_key = Some(mask);
                            self.state = DecodeState::MASK;
                        } else {
                            return Err(util::other("invalid mask value"));
                        }
                    } else {
                        self.mask_key = None;
                        self.state = DecodeState::MASK;
                    }
                }
                DecodeState::MASK => {
                    self.min_len += self.payload_length;
                    #[cfg_attr(feature = "cargo-clippy", allow(cast_possible_truncation))]
                    let size = self.min_len as usize;
                    if buf_len < size {
                        self.min_len -= self.payload_length;
                        return Ok(None);
                    }

                    if self.payload_length > 125 && self.opcode.is_control() {
                        return Err(util::other("invalid control frame"));
                    }
                    if self.payload_length > 0 {
                        #[cfg_attr(feature = "cargo-clippy", allow(cast_possible_truncation))]
                        let mut app_data_bytes = buf.drain_to(self.payload_length as usize);
                        let mut adb = app_data_bytes.get_mut();
                        if let Some(mask_key) = self.mask_key {
                            try!(self.apply_mask(&mut adb, mask_key));
                            self.application_data = Some(adb.to_vec());
                            self.state = DecodeState::FULL;
                        } else {
                            return Err(util::other("cannot unmask data"));
                        }
                    } else {
                        self.state = DecodeState::FULL;
                    }
                }
                DecodeState::FULL => break,
            }
        }

        if self.opcode == OpCode::Text && self.fin {
            if let Some(ref app_data) = self.application_data {
                try!(String::from_utf8(app_data.clone())
                    .map_err(|_| util::other("invalid utf8 in text frame")));
            }
        }

        Ok(Some(self.clone().into()))
    }

    fn encode(&mut self, msg: Self::Out, buf: &mut Vec<u8>) -> io::Result<()> {
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
        buf.push(first_byte);

        let mut second_byte = 0_u8;
        let len = msg.payload_length();
        if len < TWO_EXT as u64 {
            #[cfg_attr(feature = "cargo-clippy", allow(cast_possible_truncation))]
            let cast_len = len as u8;
            second_byte |= cast_len;
            buf.push(second_byte);
        } else if len < 65536 {
            second_byte |= TWO_EXT;
            let mut len_buf = Vec::with_capacity(2);
            try!(len_buf.write_u16::<BigEndian>(len as u16));
            buf.push(second_byte);
            buf.extend(len_buf);
        } else {
            second_byte |= EIGHT_EXT;
            let mut len_buf = Vec::with_capacity(8);
            try!(len_buf.write_u64::<BigEndian>(len));
            buf.push(second_byte);
            buf.extend(len_buf);
        }

        if let Some(app_data) = msg.application_data() {
            buf.extend(app_data);
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
        frame.set_opcode(frame_codec.opcode);
        frame.set_payload_length(frame_codec.payload_length);
        frame.set_application_data(frame_codec.application_data);
        frame.set_extension_data(frame_codec.extension_data);
        frame
    }
}

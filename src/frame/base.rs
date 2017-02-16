//! A websocket base frame
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
#[cfg(feature = "pmdeflate")]
use flate2::Compression;
#[cfg(feature = "pmdeflate")]
use flate2::write::DeflateEncoder;
#[cfg(feature = "pmdeflate")]
use inflate::InflateStream;
use std::fmt;
use std::io::{self, Cursor};
#[cfg(feature = "pmdeflate")]
use std::io::Write;
use tokio_core::io::EasyBuf;
use util;

/// Operation codes as part of rfc6455.
/// Taken from [ws-rs](https://github.com/housleyjk/ws-rs)
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
    Reserved,
    /// Indicates an invalid opcode was received.
    Bad,
}

impl OpCode {
    /// Is this a control opcode?
    pub fn is_control(&self) -> bool {
        match *self {
            OpCode::Close | OpCode::Ping | OpCode::Pong => true,
            _ => false,
        }
    }

    /// Is this opcode reserved or bad?
    pub fn is_invalid(&self) -> bool {
        match *self {
            OpCode::Reserved | OpCode::Bad => true,
            _ => false,
        }
    }
}

impl fmt::Display for OpCode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            OpCode::Continue => write!(f, "Continue"),
            OpCode::Text => write!(f, "Text"),
            OpCode::Binary => write!(f, "Binary"),
            OpCode::Close => write!(f, "Close"),
            OpCode::Ping => write!(f, "Ping"),
            OpCode::Pong => write!(f, "Pong"),
            OpCode::Reserved => write!(f, "Reserved"),
            OpCode::Bad => write!(f, "Bad"),
        }
    }
}

impl From<u8> for OpCode {
    fn from(val: u8) -> OpCode {
        match val {
            0 => OpCode::Continue,
            1 => OpCode::Text,
            2 => OpCode::Binary,
            8 => OpCode::Close,
            9 => OpCode::Ping,
            10 => OpCode::Pong,
            3 | 4 | 5 | 6 | 7 | 11 | 12 | 13 | 14 | 15 => OpCode::Reserved,
            _ => OpCode::Bad,
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
            OpCode::Reserved | OpCode::Bad => 3,
        }
    }
}

/// If the payload length byte is 126, the following two bytes represent the actual payload
/// length.
const TWO_EXT: u8 = 126;
/// If the payload length byte is 127, the following eight bytes represent the actual payload
/// length.
const EIGHT_EXT: u8 = 127;

/// A struct representing a websocket frame.
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
    /// Is the deflate extension enabled?
    deflate: bool,
    /// Has this frame been inflated?
    inflated: bool,
}

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
    // /// The data has been decoded.
    // DATA,
    /// The decoding is complete.
    FULL,
}

impl Frame {
    /// Set the `deflate` flag.
    pub fn set_deflate(&mut self, deflate: bool) -> &mut Frame {
        self.deflate = deflate;
        self
    }

    /// Get the `fin` flag.
    pub fn fin(&self) -> bool {
        self.fin
    }

    /// Set the `fin` flag.
    pub fn set_fin(&mut self, fin: bool) -> &mut Frame {
        self.fin = fin;
        self
    }

    /// Get the `rsv1` flag.
    pub fn rsv1(&self) -> bool {
        self.rsv1
    }

    /// Set the `rsv1` flag.
    pub fn set_rsv1(&mut self, rsv1: bool) -> &mut Frame {
        self.rsv1 = rsv1;
        self
    }

    /// Get the `rsv2` flag.
    pub fn rsv2(&self) -> bool {
        self.rsv2
    }

    /// Set the `rsv2` flag.
    pub fn set_rsv2(&mut self, rsv2: bool) -> &mut Frame {
        self.rsv2 = rsv2;
        self
    }

    /// Get the `rsv3` flag.
    pub fn rsv3(&self) -> bool {
        self.rsv3
    }

    /// Set the `rsv3` flag.
    pub fn set_rsv3(&mut self, rsv3: bool) -> &mut Frame {
        self.rsv3 = rsv3;
        self
    }

    /// Get the `opcode`.
    pub fn opcode(&self) -> OpCode {
        self.opcode
    }

    /// Set the `opcode`
    pub fn set_opcode(&mut self, opcode: OpCode) -> &mut Frame {
        self.opcode = opcode;
        self
    }

    /// Get the `masked` flag.
    pub fn masked(&self) -> bool {
        self.masked
    }

    /// Set the `masked` flag.
    pub fn set_masked(&mut self, masked: bool) -> &mut Frame {
        self.masked = masked;
        self
    }

    /// Get the `payload_length`.
    pub fn payload_length(&self) -> u64 {
        self.payload_length
    }

    /// Set the `payload_length`
    pub fn set_payload_length(&mut self, payload_length: u64) -> &mut Frame {
        self.payload_length = payload_length;
        self
    }

    /// Get the `mask_key`.
    pub fn mask_key(&self) -> Option<u32> {
        self.mask_key
    }

    /// Set the `mask_key`
    pub fn set_mask_key(&mut self, mask_key: Option<u32>) -> &mut Frame {
        self.mask_key = mask_key;
        self
    }

    /// Get the `extension_data`.
    pub fn extension_data(&self) -> Option<&Vec<u8>> {
        if let Some(ref ed) = self.extension_data {
            Some(ed)
        } else {
            None
        }
    }

    /// Set the `extension_data`.
    pub fn set_extension_data(&mut self, extension_data: Option<Vec<u8>>) -> &mut Frame {
        self.extension_data = extension_data;
        self
    }

    /// Get the `application_data`
    pub fn application_data(&self) -> Option<&Vec<u8>> {
        if let Some(ref ad) = self.application_data {
            Some(ad)
        } else {
            None
        }
    }

    /// Set the `application_data`
    pub fn set_application_data(&mut self, application_data: Option<Vec<u8>>) -> &mut Frame {
        self.application_data = application_data;
        self
    }

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

    /// Decode an `EasyBuf` buffer into a `Frame`
    /// Inspired by [ws-rs](https://github.com/housleyjk/ws-rs)
    pub fn decode(&mut self, buf: &mut EasyBuf) -> Result<Option<Frame>, io::Error> {
        let buf_len = buf.len();
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
                    if self.rsv1 && !self.deflate {
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

        if self.deflate && !self.opcode.is_control() && !self.inflated && self.fin {
            try!(self.inflate());
        }

        if self.opcode == OpCode::Text && self.fin {
            if let Some(ref app_data) = self.application_data {
                try!(String::from_utf8(app_data.clone())
                    .map_err(|_| util::other("invalid utf8 in text frame")));
            }
        }

        Ok(Some(self.clone()))
    }

    #[cfg(feature = "pmdeflate")]
    /// Uncompress the application data in the given base frame.
    fn inflate(&mut self) -> io::Result<()> {
        let mut buf = Vec::new();
        // let mut total = 0;
        if let Some(ref app_data) = self.application_data {
            // let len = app_data.len();
            let mut inflater = InflateStream::new();
            let mut n = 0;
            // let trimmed: Vec<u8> = app_data.iter().filter(|b| **b != 0x00).cloned().collect();
            let mut extended = app_data.clone();
            extended.extend(&[0x00, 0x00, 0xff, 0xff]);
            while n < extended.len() {
                let res = inflater.update(&extended[n..]);
                if let Ok((num_bytes_read, result)) = res {
                    n += num_bytes_read;
                    buf.extend(result);
                } else {
                    res.expect("Unable to inflate buffer");
                }
            }
        }

        if !buf.is_empty() {
            // let base = buf[0..total].to_vec();
            self.inflated = true;
            self.rsv1 = false;
            self.payload_length = buf.len() as u64;
            self.application_data = Some(buf);
        }
        Ok(())
    }

    #[cfg(not(feature = "pmdeflate"))]
    /// Does nothing when `pmdeflate` feature is disabled.
    fn inflate(&mut self) -> io::Result<()> {
        Ok(())
    }

    /// Convert a `Frame` into a buffer of bytes.
    /// Inspired by [ws-rs](https://github.com/housleyjk/ws-rs)
    pub fn as_byte_buf(&mut self, buf: &mut Vec<u8>) -> Result<(), io::Error> {
        if self.deflate && !self.opcode.is_control() && self.fin {
            try!(self.deflate());
        }
        let mut first_byte = 0_u8;

        if self.fin {
            first_byte |= 0x80;
        }

        if self.rsv1 {
            first_byte |= 0x40;
        }

        if self.rsv2 {
            first_byte |= 0x20;
        }

        if self.rsv3 {
            first_byte |= 0x10;
        }

        let opcode: u8 = self.opcode.into();
        first_byte |= opcode;

        buf.push(first_byte);

        let mut second_byte = 0_u8;
        if self.masked {
            second_byte |= 0x80;
        }

        let len = self.payload_length;
        if len < 126 {
            #[cfg_attr(feature = "cargo-clippy", allow(cast_possible_truncation))]
            let cast_len = len as u8;
            second_byte |= cast_len;
            buf.push(second_byte);
        } else if len < 65536 {
            second_byte |= 126;
            let mut len_buf = Vec::with_capacity(2);
            try!(len_buf.write_u16::<BigEndian>(len as u16));
            buf.push(second_byte);
            buf.extend(len_buf);
        } else {
            second_byte |= 127;
            let mut len_buf = Vec::with_capacity(8);
            try!(len_buf.write_u64::<BigEndian>(len));
            buf.push(second_byte);
            buf.extend(len_buf);
        }

        if let Some(ref app_data) = self.application_data {
            buf.extend(app_data);
        }

        Ok(())
    }

    #[cfg(feature = "pmdeflate")]
    /// Compress the application data in the given base frame.
    fn deflate(&mut self) -> io::Result<()> {
        let mut compressed = Vec::<u8>::new();
        if let Some(app_data) = self.application_data() {
            let mut encoder = DeflateEncoder::new(Vec::new(), Compression::Default);
            try!(encoder.write_all(app_data));
            if let Ok(encoded_data) = encoder.finish() {
                compressed.extend(encoded_data);
                // compressed.extend(&[0x00, 0x00, 0xff, 0xff]);
            }
        }

        if compressed.is_empty() {
            self.application_data = None;
        } else {
            self.rsv1 = true;
            self.payload_length = compressed.len() as u64;
            self.application_data = Some(compressed);
        }

        Ok(())
    }

    #[cfg(not(feature = "pmdeflate"))]
    /// Does nothing when `pmdeflate` feature is disabled.
    fn deflate(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Default for Frame {
    fn default() -> Frame {
        Frame {
            fin: true,
            rsv1: false,
            rsv2: false,
            rsv3: false,
            opcode: OpCode::Close,
            masked: false,
            length_code: 0,
            payload_length: 0,
            mask_key: None,
            extension_data: None,
            application_data: None,
            state: DecodeState::NONE,
            min_len: 2,
            deflate: false,
            inflated: false,
        }
    }
}

impl fmt::Display for Frame {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        try!(write!(f, "Frame {{"));
        try!(write!(f, "\n\tfin: {}", self.fin));
        try!(write!(f, "\n\trsv1: {}", self.rsv1));
        try!(write!(f, "\n\trsv2: {}", self.rsv2));
        try!(write!(f, "\n\trsv3 {}", self.rsv3));
        try!(write!(f, "\n\trsv3 {}", self.rsv3));
        try!(write!(f, "\n\topcode {}", self.opcode));
        try!(write!(f, "\n\tmasked {}", self.masked));
        try!(write!(f, "\n\tpayload_length {}", self.payload_length));
        if let Some(ref ext_data) = self.extension_data {
            try!(write!(f, "\n\textension_data\n{}", util::as_hex(ext_data)));
        }
        if let Some(ref app_data) = self.application_data {
            try!(write!(f, "\n\tapplication_data\n{}\n", util::as_hex(app_data)));
        }
        writeln!(f, "}}")
    }
}

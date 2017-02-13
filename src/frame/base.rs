//! A websocket base frame
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use std::fmt;
use std::io::{self, Cursor};
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
    /// The `payload_length`
    payload_length: u64,
    /// The optional `mask`
    mask_key: Option<u32>,
    /// The optional `extension_data`
    extension_data: Option<Vec<u8>>,
    /// The optional `application_data`
    application_data: Option<Vec<u8>>,
}

impl Frame {
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
        // Split of the 2 'header' bytes.
        let header_bytes = buf.drain_to(2);
        let header = header_bytes.as_slice();
        let first = header[0];
        let second = header[1];

        // Extract the details
        let fin = first & 0x80 != 0;
        let rsv1 = first & 0x40 != 0;
        let rsv2 = first & 0x20 != 0;
        let rsv3 = first & 0x10 != 0;
        let opcode = OpCode::from((first & 0x0F) as u8);
        let masked = second & 0x80 != 0;
        let length_code = (second & 0x7F) as u8;

        let payload_length = if length_code == TWO_EXT {
            let mut rdr = Cursor::new(buf.drain_to(2));
            if let Ok(len) = rdr.read_u16::<BigEndian>() {
                len as u64
            } else {
                return Ok(None);
            }
        } else if length_code == EIGHT_EXT {
            let mut rdr = Cursor::new(buf.drain_to(8));
            if let Ok(len) = rdr.read_u64::<BigEndian>() {
                len
            } else {
                return Ok(None);
            }
        } else {
            length_code as u64
        };

        let mask_key = if masked {
            let mut rdr = Cursor::new(buf.drain_to(4));
            if let Ok(mask) = rdr.read_u32::<BigEndian>() {
                Some(mask)
            } else {
                None
            }
        } else {
            None
        };

        #[cfg_attr(feature = "cargo-clippy", allow(cast_possible_truncation))]
        let mut app_data_bytes = buf.drain_to(payload_length as usize);
        let mut adb = app_data_bytes.get_mut();
        let application_data = if let Some(mask_key) = mask_key {
            try!(self.apply_mask(&mut adb, mask_key));
            Some(adb.to_vec())
        } else {
            None
        };

        let base_frame = Frame {
            fin: fin,
            rsv1: rsv1,
            rsv2: rsv2,
            rsv3: rsv3,
            opcode: opcode,
            masked: masked,
            payload_length: payload_length,
            mask_key: mask_key,
            application_data: application_data,
            ..Default::default()
        };

        Ok(Some(base_frame))
    }

    /// Convert a `Frame` into a buffer of bytes.
    /// Inspired by [ws-rs](https://github.com/housleyjk/ws-rs)
    pub fn to_byte_buf(&self, buf: &mut Vec<u8>) -> Result<(), io::Error> {
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

        if let (true, Some(mask)) = (self.masked, self.mask_key) {
            let mut mask_buf = Vec::with_capacity(4);
            try!(mask_buf.write_u32::<BigEndian>(mask));
            buf.extend(mask_buf);
        }

        if let Some(ref app_data) = self.application_data {
            buf.extend(app_data);
        }

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
            payload_length: 0,
            mask_key: None,
            extension_data: None,
            application_data: None,
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

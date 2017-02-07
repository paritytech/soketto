use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use std::fmt;
use std::io::{self, Cursor};
use tokio_core::io::EasyBuf;

/// Operation codes as part of rfc6455.
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
    pub fn is_control(&self) -> bool {
        match *self {
            OpCode::Close | OpCode::Ping | OpCode::Pong => true,
            _ => false,
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
            OpCode::Reserved => 3,
            OpCode::Bad => 3,
        }
    }
}

const TWO_EXT: u8 = 126;
const EIGHT_EXT: u8 = 127;

/// A struct representing a websocket frame.
#[derive(Debug, Clone)]
pub struct BaseFrame {
    fin: bool,
    rsv1: bool,
    rsv2: bool,
    rsv3: bool,
    opcode: OpCode,
    masked: bool,
    payload_length: u64,
    mask_key: Option<u32>,
    extension_data: Option<Vec<u8>>,
    application_data: Option<Vec<u8>>,
}

impl BaseFrame {
    pub fn fin(&self) -> bool {
        self.fin
    }

    pub fn set_fin(&mut self, fin: bool) -> &mut BaseFrame {
        self.fin = fin;
        self
    }

    pub fn rsv1(&self) -> bool {
        self.rsv1
    }

    pub fn set_rsv1(&mut self, rsv1: bool) -> &mut BaseFrame {
        self.rsv1 = rsv1;
        self
    }

    pub fn rsv2(&self) -> bool {
        self.rsv2
    }

    pub fn set_rsv2(&mut self, rsv2: bool) -> &mut BaseFrame {
        self.rsv2 = rsv2;
        self
    }

    pub fn rsv3(&self) -> bool {
        self.rsv3
    }

    pub fn set_rsv3(&mut self, rsv3: bool) -> &mut BaseFrame {
        self.rsv3 = rsv3;
        self
    }

    pub fn opcode(&self) -> OpCode {
        self.opcode
    }

    pub fn set_opcode(&mut self, opcode: OpCode) -> &mut BaseFrame {
        self.opcode = opcode;
        self
    }

    pub fn masked(&self) -> bool {
        self.masked
    }

    pub fn set_masked(&mut self, masked: bool) -> &mut BaseFrame {
        self.masked = masked;
        self
    }

    pub fn payload_length(&self) -> u64 {
        self.payload_length
    }

    pub fn set_payload_length(&mut self, payload_length: u64) -> &mut BaseFrame {
        self.payload_length = payload_length;
        self
    }

    pub fn mask_key(&self) -> Option<u32> {
        self.mask_key
    }

    pub fn set_mask_key(&mut self, mask_key: Option<u32>) -> &mut BaseFrame {
        self.mask_key = mask_key;
        self
    }

    pub fn extension_data(&self) -> Option<&Vec<u8>> {
        if let Some(ref ed) = self.extension_data {
            Some(ed)
        } else {
            None
        }
    }

    pub fn set_extension_data(&mut self, extension_data: Option<Vec<u8>>) -> &mut BaseFrame {
        self.extension_data = extension_data;
        self
    }

    pub fn application_data(&self) -> Option<&Vec<u8>> {
        if let Some(ref ad) = self.application_data {
            Some(ad)
        } else {
            None
        }
    }

    pub fn set_application_data(&mut self, application_data: Option<Vec<u8>>) -> &mut BaseFrame {
        self.application_data = application_data;
        self
    }

    pub fn app_data(&self) -> &[u8] {
        if let Some(ref data) = self.application_data {
            data
        } else {
            &[]
        }
    }

    pub fn decode(&mut self, buf: &mut EasyBuf) -> Result<Option<BaseFrame>, io::Error> {
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
            if let Ok(mask_key) = rdr.read_u32::<BigEndian>() {
                Some(mask_key)
            } else {
                return Ok(None);
            }
        } else {
            None
        };

        let rest_len = buf.len();
        let app_data_bytes = buf.drain_to(rest_len);
        let application_data = Some(app_data_bytes.as_slice().to_vec());

        let base_frame = BaseFrame {
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

        // if let Some(ref stdout) = self.stdout {
        //     trace!(stdout, "decoded base_frame: {}", base_frame);
        // }

        Ok(Some(base_frame))
    }

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
            second_byte |= len as u8;
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

        // if let Some(ref stdout) = self.stdout {
        //     trace!(stdout, "write buf: {:?}", buf);
        // }

        Ok(())
    }
}

impl Default for BaseFrame {
    fn default() -> BaseFrame {
        BaseFrame {
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

impl fmt::Display for BaseFrame {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f,
               "BaseFrame {{\n\tfin: {}\n\trsv1: {}\n\trsv2: {}\n\trsv3: {}\n\topcode: {:?}\
               \n\tmasked: {}\n\tpayload_length: {}\n\tmask_key: {:?}\n\textension_data: {:?}\
               \n\tapplication_data: {:?}\n}}",
               self.fin,
               self.rsv1,
               self.rsv2,
               self.rsv3,
               self.opcode,
               self.masked,
               self.payload_length,
               self.mask_key,
               self.extension_data,
               self.application_data)
    }
}

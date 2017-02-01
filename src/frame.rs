use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use opcode::OpCode;
use std::convert::From;
use std::io::{self, Cursor};
use tokio_core::io::{Codec, EasyBuf};

const TWO_EXT: u8 = 126;
const EIGHT_EXT: u8 = 127;

/// A struct representing a websocket frame.
#[derive(Debug, Clone)]
pub struct Frame {
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

#[allow(dead_code)]
pub struct FrameCodec;

impl Codec for FrameCodec {
    type In = Frame;
    type Out = Frame;

    fn decode(&mut self, buf: &mut EasyBuf) -> Result<Option<Frame>, io::Error> {
        // Split of the 2 'header' bytes.
        let mut rest = buf.split_off(2);
        let header = buf.as_slice();
        let first = header[0];
        let second = header[1];

        // Extract the details
        let fin = first & 0x80 != 0;
        let rsv1 = first & 0x40 != 0;
        let rsv2 = first & 0x20 != 0;
        let rsv3 = first & 0x10 != 0;
        let opcode = (first & 0x0F) as u8;
        let masked = second & 0x80 != 0;
        let length_code = (second & 0x7F) as u8;

        let payload_length = if length_code == TWO_EXT {
            let mut rdr = Cursor::new(rest.drain_to(2));
            if let Ok(len) = rdr.read_u16::<BigEndian>() {
                len as u64
            } else {
                return Ok(None);
            }
        } else if length_code == EIGHT_EXT {
            let mut rdr = Cursor::new(rest.drain_to(8));
            if let Ok(len) = rdr.read_u64::<BigEndian>() {
                len
            } else {
                return Ok(None);
            }
        } else {
            length_code as u64
        };

        let mask_key = if masked {
            let mut rdr = Cursor::new(rest.drain_to(4));
            if let Ok(mask_key) = rdr.read_u32::<BigEndian>() {
                Some(mask_key)
            } else {
                return Ok(None);
            }
        } else {
            None
        };

        let application_data = Some(rest.as_slice().to_vec());

        Ok(Some(Frame {
            fin: fin,
            rsv1: rsv1,
            rsv2: rsv2,
            rsv3: rsv3,
            opcode: OpCode::from(opcode),
            masked: masked,
            payload_length: payload_length,
            mask_key: mask_key,
            application_data: application_data,
            ..Default::default()
        }))
    }

    fn encode(&mut self, msg: Frame, buf: &mut Vec<u8>) -> io::Result<()> {
        let mut first_byte = 0_u8;

        if msg.fin {
            first_byte |= 0x80;
        }

        if msg.rsv1 {
            first_byte |= 0x40;
        }

        if msg.rsv2 {
            first_byte |= 0x20;
        }

        if msg.rsv3 {
            first_byte |= 0x10;
        }

        let opcode: u8 = msg.opcode.into();
        first_byte |= opcode;

        buf.push(first_byte);

        let mut second_byte = 0_u8;
        if msg.masked {
            second_byte |= 0x80;
        }

        let len = msg.payload_length;
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

        if let (true, Some(mask)) = (msg.masked, msg.mask_key) {
            let mut mask_buf = Vec::with_capacity(4);
            try!(mask_buf.write_u32::<BigEndian>(mask));
            buf.extend(mask_buf);
        }

        if let Some(app_data) = msg.application_data {
            buf.extend(app_data);
        }

        Ok(())
    }
}


#[cfg(test)]
mod test {
    use frame::{Frame, FrameCodec};
    use opcode::OpCode;
    use tokio_core::io::{Codec, EasyBuf};
    use util;

    #[cfg_attr(rustfmt, rustfmt_skip)]
    const SHORT:  [u8; 7]   = [0x88, 0x81, 0x00, 0x00, 0x00, 0x01, 0x00];
    #[cfg_attr(rustfmt, rustfmt_skip)]
    const MID:    [u8; 134] = [0x88, 0xFE, 0x00, 0x7E, 0x00, 0x00, 0x00, 0x01,
                               0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                               0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                               0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                               0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                               0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                               0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                               0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                               0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                               0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                               0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                               0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                               0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                               0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                               0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                               0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                               0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
    #[cfg_attr(rustfmt, rustfmt_skip)]
    const LONG:   [u8; 15]  = [0x88, 0xFF, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01,
                               0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00];
    #[cfg_attr(rustfmt, rustfmt_skip)]
    const CONT:   [u8; 7]   = [0x80, 0x81, 0x00, 0x00, 0x00, 0x01, 0x00];
    #[cfg_attr(rustfmt, rustfmt_skip)]
    const TEXT:   [u8; 7]   = [0x81, 0x81, 0x00, 0x00, 0x00, 0x01, 0x00];
    #[cfg_attr(rustfmt, rustfmt_skip)]
    const BINARY: [u8; 7]   = [0x82, 0x81, 0x00, 0x00, 0x00, 0x01, 0x00];
    #[cfg_attr(rustfmt, rustfmt_skip)]
    const PING:   [u8; 7]   = [0x89, 0x81, 0x00, 0x00, 0x00, 0x01, 0x00];
    #[cfg_attr(rustfmt, rustfmt_skip)]
    const PONG:   [u8; 7]   = [0x8A, 0x81, 0x00, 0x00, 0x00, 0x01, 0x00];
    #[cfg_attr(rustfmt, rustfmt_skip)]
    const RES:    [u8; 7]   = [0x83, 0x81, 0x00, 0x00, 0x00, 0x01, 0x00];

    fn decode_test(vec: Vec<u8>, opcode: OpCode, masked: bool, len: u64, mask: Option<u32>) {
        let mut eb = EasyBuf::from(vec);
        let mut fc = FrameCodec;
        if let Ok(Some(decoded)) = <FrameCodec as Codec>::decode(&mut fc, &mut eb) {
            assert!(decoded.fin);
            assert!(!decoded.rsv1);
            assert!(!decoded.rsv2);
            assert!(!decoded.rsv3);
            assert!(decoded.opcode == opcode);
            assert!(decoded.masked == masked);
            assert!(decoded.payload_length == len);
            assert!(decoded.mask_key == mask);
            assert!(decoded.extension_data.is_none());
            assert!(decoded.application_data.is_some());
        } else {
            assert!(false);
        }
    }

    fn encode_test(cmp: Vec<u8>,
                   opcode: OpCode,
                   len: u64,
                   masked: bool,
                   mask: Option<u32>,
                   app_data: Option<Vec<u8>>) {
        let mut fc = FrameCodec;
        let mut frame: Frame = Default::default();
        frame.opcode = opcode;
        frame.payload_length = len;
        frame.masked = masked;
        frame.mask_key = mask;
        frame.application_data = app_data;
        let mut buf = vec![];
        if let Ok(()) = <FrameCodec as Codec>::encode(&mut fc, frame, &mut buf) {
            println!("{}", util::as_hex(&buf));
            assert!(buf.len() == cmp.len());
            for (a, b) in buf.iter().zip(cmp.iter()) {
                assert!(a == b);
            }
        }
    }

    #[test]
    fn decode() {
        decode_test(SHORT.to_vec(), OpCode::Close, true, 1, Some(1));
        decode_test(MID.to_vec(), OpCode::Close, true, 126, Some(1));
        decode_test(LONG.to_vec(), OpCode::Close, true, 65536, Some(1));
        decode_test(CONT.to_vec(), OpCode::Continue, true, 1, Some(1));
        decode_test(TEXT.to_vec(), OpCode::Text, true, 1, Some(1));
        decode_test(BINARY.to_vec(), OpCode::Binary, true, 1, Some(1));
        decode_test(PING.to_vec(), OpCode::Ping, true, 1, Some(1));
        decode_test(PONG.to_vec(), OpCode::Pong, true, 1, Some(1));
        decode_test(RES.to_vec(), OpCode::Reserved, true, 1, Some(1));
    }

    #[test]
    fn encode() {
        encode_test(SHORT.to_vec(),
                    OpCode::Close,
                    1,
                    true,
                    Some(1),
                    Some(vec![0]));
        encode_test(MID.to_vec(),
                    OpCode::Close,
                    126,
                    true,
                    Some(1),
                    Some(vec![0; 126]));
        encode_test(LONG.to_vec(),
                    OpCode::Close,
                    65536,
                    true,
                    Some(1),
                    Some(vec![0]));
        encode_test(CONT.to_vec(),
                    OpCode::Continue,
                    1,
                    true,
                    Some(1),
                    Some(vec![0]));
        encode_test(TEXT.to_vec(), OpCode::Text, 1, true, Some(1), Some(vec![0]));
        encode_test(BINARY.to_vec(),
                    OpCode::Binary,
                    1,
                    true,
                    Some(1),
                    Some(vec![0]));
        encode_test(PING.to_vec(), OpCode::Ping, 1, true, Some(1), Some(vec![0]));
        encode_test(PONG.to_vec(), OpCode::Pong, 1, true, Some(1), Some(vec![0]));
        encode_test(RES.to_vec(),
                    OpCode::Reserved,
                    1,
                    true,
                    Some(1),
                    Some(vec![0]));
    }
}

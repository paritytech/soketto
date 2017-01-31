use byteorder::{BigEndian, ReadBytesExt};
use opcode::OpCode;
use std::convert::From;
use std::io::{self, Cursor};
use tokio_core::io::{Codec, EasyBuf};

const TWO_EXT: u8 = 126;
const EIGHT_EXT: u8 = 127;

/// A struct representing a WebSocket frame.
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

impl Codec for Frame {
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

    fn encode(&mut self, _msg: Frame, _buf: &mut Vec<u8>) -> io::Result<()> {
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

#[cfg(test)]
mod test {
    use frame::Frame;
    use opcode::OpCode;
    use tokio_core::io::{Codec, EasyBuf};

    #[cfg_attr(rustfmt, rustfmt_skip)]
    const SHORT: [u8; 7] = [0x88, 0x81, 0x00, 0x00, 0x00, 0x01, 0x00];
    #[cfg_attr(rustfmt, rustfmt_skip)]
    const MID: [u8; 134] = [0x88, 0xFE, 0x00, 0x7E, 0x00, 0x00, 0x00, 0x01,
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
    // static ref LONG: Vec<u8>  = vec![0b10001111, 0b11111111,
    //                                  0b00000000, 0b00000000,
    //                                  0b00000000, 0b00000000,
    //                                  0b00000000, 0b10000000,
    //                                  0b00000000, 0b00000000];

    #[test]
    fn decode_short() {
        let mut ebs = EasyBuf::from(SHORT.to_vec());
        let mut frame: Frame = Default::default();
        if let Ok(Some(decoded)) = <Frame as Codec>::decode(&mut frame, &mut ebs) {
            assert!(decoded.fin);
            assert!(!decoded.rsv1);
            assert!(!decoded.rsv2);
            assert!(!decoded.rsv3);
            assert!(decoded.opcode == OpCode::Close);
            assert!(decoded.masked);
            assert!(decoded.payload_length == 1);
            assert!(decoded.mask_key == Some(1));
            assert!(decoded.extension_data.is_none());

            if let Some(application_data) = decoded.application_data {
                assert!(application_data.len() == 1);
            } else {
                assert!(false);
            }
        } else {
            assert!(false);
        }
    }

    #[test]
    fn decode_mid() {
        let mut ebs = EasyBuf::from(MID.to_vec());
        let mut frame: Frame = Default::default();
        if let Ok(Some(decoded)) = <Frame as Codec>::decode(&mut frame, &mut ebs) {
            assert!(decoded.fin);
            assert!(!decoded.rsv1);
            assert!(!decoded.rsv2);
            assert!(!decoded.rsv3);
            assert!(decoded.opcode == OpCode::Close);
            assert!(decoded.masked);
            assert!(decoded.payload_length == 126);
            assert!(decoded.mask_key == Some(1));
            assert!(decoded.extension_data.is_none());

            if let Some(application_data) = decoded.application_data {
                assert!(application_data.len() == 126);
            } else {
                assert!(false);
            }
        } else {
            assert!(false);
        }
    }
}

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use opcode::OpCode;
use std::convert::From;
use std::fmt;
use std::io::{self, Cursor};
use tokio_core::io::{Codec, EasyBuf};
use tokio_proto::streaming::pipeline::Frame;

const TWO_EXT: u8 = 126;
const EIGHT_EXT: u8 = 127;

fn other(desc: &str) -> io::Error {
    io::Error::new(io::ErrorKind::Other, desc)
}

/// A struct representing a websocket frame.
#[derive(Debug, Clone)]
pub struct WebsocketFrame {
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

impl WebsocketFrame {
    pub fn opcode(&self) -> OpCode {
        self.opcode
    }

    pub fn app_data(&self) -> &[u8] {
        if let Some(ref data) = self.application_data {
            data
        } else {
            &[]
        }
    }
}

impl Default for WebsocketFrame {
    fn default() -> WebsocketFrame {
        WebsocketFrame {
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

impl fmt::Display for WebsocketFrame {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f,
               "WebsocketFrame {{ fin: {} rsv1: {} rsv2: {} rsv3: {} opcode: {:?} masked: {} \
               payload_length: {} mask_key: {:?} extension_data: {:?}  application_data: {:?} }}",
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

pub struct FrameCodec {
    fragmented: bool,
}

impl Default for FrameCodec {
    fn default() -> FrameCodec {
        FrameCodec { fragmented: false }
    }
}

fn to_byte_buf(frame: WebsocketFrame, buf: &mut Vec<u8>) -> Result<(), io::Error> {
    let mut first_byte = 0_u8;

    if frame.fin {
        first_byte |= 0x80;
    }

    if frame.rsv1 {
        first_byte |= 0x40;
    }

    if frame.rsv2 {
        first_byte |= 0x20;
    }

    if frame.rsv3 {
        first_byte |= 0x10;
    }

    let opcode: u8 = frame.opcode.into();
    first_byte |= opcode;

    buf.push(first_byte);

    let mut second_byte = 0_u8;
    if frame.masked {
        second_byte |= 0x80;
    }

    println!("second byte: {}", second_byte);
    let len = frame.payload_length;
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

    if let (true, Some(mask)) = (frame.masked, frame.mask_key) {
        let mut mask_buf = Vec::with_capacity(4);
        try!(mask_buf.write_u32::<BigEndian>(mask));
        buf.extend(mask_buf);
    }

    if let Some(app_data) = frame.application_data {
        buf.extend(app_data);
    }

    println!("write buf: {:?}", buf);
    Ok(())
}

impl Codec for FrameCodec {
    type In = Frame<WebsocketFrame, WebsocketFrame, io::Error>;
    type Out = Frame<WebsocketFrame, WebsocketFrame, io::Error>;

    fn decode(&mut self, buf: &mut EasyBuf) -> Result<Option<Self::In>, io::Error> {
        if buf.len() == 0 {
            return Ok(None);
        }

        // Split of the 2 'header' bytes.
        let header_bytes = buf.drain_to(2);
        println!("post header buf len: {}", buf.len());
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

        println!("post payload_len calc buf len: {}", buf.len());
        println!("rest: {:?}, masked: {}", buf, masked);

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

        println!("post mask_key buf len: {}", buf.len());
        let rest_len = buf.len();
        let app_data_bytes = buf.drain_to(rest_len);
        let application_data = Some(app_data_bytes.as_slice().to_vec());

        let ws_frame = WebsocketFrame {
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

        println!("decode ws_frame: {}", ws_frame);

        // Control frames (see Section 5.5) MAY be injected in the middle of
        // a fragmented message.  Control frames themselves MUST NOT be
        // fragmented.
        if opcode.is_control() {
            Ok(Some(Frame::Message {
                message: ws_frame,
                body: false,
            }))
        }
        // An unfragmented message consists of a single frame with the FIN
        // bit set (Section 5.2) and an opcode other than 0.
        else if !self.fragmented && fin && opcode != OpCode::Continue {
            Ok(Some(Frame::Message {
                message: ws_frame,
                body: false,
            }))
        }
        // A fragmented message consists of a single frame with the FIN bit
        // clear and an opcode other than 0, followed by zero or more frames
        // with the FIN bit clear and the opcode set to 0, and terminated by
        // a single frame with the FIN bit set and an opcode of 0.
        //
        // The following case handles the first message of a fragmented chain, where
        // we have set the fragmented flag, the fin bit is clear, and the opcode
        // is not Continue.
        else if !self.fragmented && !fin && opcode != OpCode::Continue {
            self.fragmented = true;
            Ok(Some(Frame::Message {
                message: ws_frame,
                body: true,
            }))
        }
        // The following case handles intemediate frames of a fragment chain,
        // where the fin bit is clear, and the opcode is Continue.
        else if self.fragmented && !fin && opcode == OpCode::Continue {
            Ok(Some(Frame::Body {
                fin: fin,
                chunk: Some(ws_frame),
            }))
        }
        // The following case handles the termination frame
        else if self.fragmented && fin && opcode == OpCode::Continue {
            self.fragmented = false;
            Ok(Some(Frame::Body {
                fin: fin,
                chunk: Some(ws_frame),
            }))
        } else {
            Err(other(&format!("Unknown frame type: {} {:?}", ws_frame.fin, ws_frame.opcode)))
        }
    }

    fn encode(&mut self, msg: Self::Out, buf: &mut Vec<u8>) -> io::Result<()> {
        match msg {
            Frame::Message { message, .. } => {
                try!(to_byte_buf(message, buf));
            }
            Frame::Body { chunk, .. } => {
                if let Some(chunk) = chunk {
                    try!(to_byte_buf(chunk, buf));
                }
            }
            Frame::Error { error } => {
                return Err(error);
            }
        }

        Ok(())
    }
}


#[cfg(test)]
mod test {
    use frame::{WebsocketFrame, FrameCodec};
    use opcode::OpCode;
    use tokio_core::io::{Codec, EasyBuf};
    use tokio_proto::streaming::pipeline::Frame;
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
    const CONT:   [u8; 7]   = [0x00, 0x81, 0x00, 0x00, 0x00, 0x01, 0x00];
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
        let mut fc = if opcode == OpCode::Continue {
            FrameCodec { fragmented: true }
        } else {
            FrameCodec { fragmented: false }
        };
        match fc.decode(&mut eb) {
            Ok(Some(decoded)) => {
                match decoded {
                    Frame::Message { message, body } => {
                        assert!(!body);
                        assert!(message.fin);
                        assert!(!message.rsv1);
                        assert!(!message.rsv2);
                        assert!(!message.rsv3);
                        assert!(message.opcode == opcode);
                        assert!(message.masked == masked);
                        assert!(message.payload_length == len);
                        assert!(message.mask_key == mask);
                        assert!(message.extension_data.is_none());
                        assert!(message.application_data.is_some());
                    }
                    Frame::Body { chunk, .. } => {
                        if let Some(chunk) = chunk {
                            assert!(!chunk.fin);
                            assert!(chunk.opcode == opcode);
                        } else {
                            assert!(false);
                        }
                    }
                    _ => {
                        assert!(false);
                    }
                }
            }
            Err(e) => {
                println!("{}", e);
                assert!(false);
            }
            _ => {
                assert!(false);
            }
        }
    }

    fn encode_test(cmp: Vec<u8>,
                   opcode: OpCode,
                   len: u64,
                   masked: bool,
                   mask: Option<u32>,
                   app_data: Option<Vec<u8>>) {
        let mut fc = FrameCodec { fragmented: false };
        let mut frame: WebsocketFrame = Default::default();
        frame.opcode = opcode;
        if opcode == OpCode::Continue {
            frame.fin = false;
        }
        frame.payload_length = len;
        frame.masked = masked;
        frame.mask_key = mask;
        frame.application_data = app_data;
        let mut buf = vec![];
        let msg = Frame::Message {
            message: frame,
            body: false,
        };
        if let Ok(()) = <FrameCodec as Codec>::encode(&mut fc, msg, &mut buf) {
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

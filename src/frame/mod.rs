use frame::base::{BaseFrame, OpCode};
use frame::handshake::HandshakeFrame;
use slog::Logger;
use std::fmt;
use std::io;
use tokio_core::io::{Codec, EasyBuf};
use tokio_proto::streaming::pipeline::Frame;

pub mod base;
pub mod handshake;

fn other(desc: &str) -> io::Error {
    io::Error::new(io::ErrorKind::Other, desc)
}

/// A struct representing a websocket frame.
#[derive(Debug, Clone)]
pub struct WebSocketFrame {
    handshake: Option<HandshakeFrame>,
    base: Option<BaseFrame>,
}

impl WebSocketFrame {
    pub fn handshake(&self) -> Option<&HandshakeFrame> {
        if let Some(ref handshake) = self.handshake {
            Some(handshake)
        } else {
            None
        }
    }

    pub fn base(&self) -> Option<&BaseFrame> {
        if let Some(ref base) = self.base {
            Some(base)
        } else {
            None
        }
    }

    pub fn set_base(&mut self, base: BaseFrame) -> &mut WebSocketFrame {
        self.base = Some(base);
        self
    }
}

impl Default for WebSocketFrame {
    fn default() -> WebSocketFrame {
        WebSocketFrame {
            handshake: None,
            base: None,
        }
    }
}

impl fmt::Display for WebSocketFrame {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let hstr = if let Some(ref handshake) = self.handshake {
            format!("{}", handshake)
        } else {
            "None".to_string()
        };

        let bstr = if let Some(ref base) = self.base {
            format!("{}", base)
        } else {
            "None".to_string()
        };

        write!(f,
               "WebSocketFrame {{\n\thandshake: {}\n\tbase: {}\n}}",
               hstr,
               bstr)
    }
}

pub struct FrameCodec {
    shaken: bool,
    fragmented: bool,
    stdout: Option<Logger>,
    stderr: Option<Logger>,
}

impl FrameCodec {
    pub fn add_stdout(&mut self, stdout: Logger) -> &mut FrameCodec {
        let fc_stdout = stdout.new(o!("module" => module_path!()));
        self.stdout = Some(fc_stdout);
        self
    }

    pub fn add_stderr(&mut self, stderr: Logger) -> &mut FrameCodec {
        let fc_stderr = stderr.new(o!("module" => module_path!()));
        self.stderr = Some(fc_stderr);
        self
    }
}

impl Default for FrameCodec {
    fn default() -> FrameCodec {
        FrameCodec {
            // TODO: Default this to false when Handshake is implemented.
            shaken: true,
            fragmented: false,
            stdout: None,
            stderr: None,
        }
    }
}

impl Codec for FrameCodec {
    type In = Frame<WebSocketFrame, WebSocketFrame, io::Error>;
    type Out = Frame<WebSocketFrame, WebSocketFrame, io::Error>;

    fn decode(&mut self, buf: &mut EasyBuf) -> Result<Option<Self::In>, io::Error> {
        if buf.len() == 0 {
            return Ok(None);
        }

        let mut ws_frame: WebSocketFrame = Default::default();

        if self.shaken {
            let mut base: BaseFrame = Default::default();
            if let Ok(Some(base)) = base.decode(buf) {
                let opcode = base.opcode();
                let fin = base.fin();

                ws_frame.set_base(base.clone());

                // Control frames (see Section 5.5) MAY be injected in the middle of
                // a fragmented message.  Control frames themselves MUST NOT be
                // fragmented.
                if opcode.is_control() && !self.fragmented {
                    if let Some(ref stdout) = self.stdout {
                        trace!(stdout, "Control Frame: {:?}", base.opcode());
                    }
                    Ok(Some(Frame::Message {
                        message: ws_frame,
                        body: false,
                    }))
                } else if opcode.is_control() && self.fragmented {
                    if let Some(ref stdout) = self.stdout {
                        trace!(stdout, "Control Frame During Stream: {:?}", base.opcode());
                    }
                    Ok(Some(Frame::Body {
                        fin: false,
                        chunk: Some(ws_frame),
                    }))
                }
                // An unfragmented message consists of a single frame with the FIN
                // bit set (Section 5.2) and an opcode other than 0.
                else if !self.fragmented && fin && opcode != OpCode::Continue {
                    if let Some(ref stdout) = self.stdout {
                        trace!(stdout, "Single Frame: {:?}", base.opcode());
                    }
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
                    if let Some(ref stdout) = self.stdout {
                        trace!(stdout, "First Frame: {:?}", base.opcode());
                    }
                    self.fragmented = true;
                    Ok(Some(Frame::Message {
                        message: ws_frame,
                        body: true,
                    }))
                }
                // The following case handles intemediate frames of a fragment chain,
                // where the fin bit is clear, and the opcode is Continue.
                else if self.fragmented && !fin && opcode == OpCode::Continue {
                    if let Some(ref stdout) = self.stdout {
                        trace!(stdout, "Intermediate Frame: {:?}", base.opcode());
                    }
                    Ok(Some(Frame::Body {
                        fin: fin,
                        chunk: Some(ws_frame),
                    }))
                }
                // The following case handles the termination frame
                else if self.fragmented && fin && opcode == OpCode::Continue {
                    if let Some(ref stdout) = self.stdout {
                        trace!(stdout, "Termination Frame: {:?}", base.opcode());
                    }
                    self.fragmented = false;
                    Ok(Some(Frame::Body {
                        fin: fin,
                        chunk: Some(ws_frame),
                    }))
                } else {
                    if let Some(ref stderr) = self.stderr {
                        error!(stderr, "Unknown frame type: {} {:?}", fin, opcode);
                    }
                    Err(other(&format!("Unknown frame type: {} {:?}", fin, opcode)))
                }
            } else {
                Ok(None)
            }
        } else {
            return Ok(None);
        }
    }

    fn encode(&mut self, msg: Self::Out, buf: &mut Vec<u8>) -> io::Result<()> {
        match msg {
            Frame::Message { message, .. } => {
                if self.shaken {
                    if let Some(base) = message.base {
                        try!(base.to_byte_buf(buf));
                    }
                }
            }
            Frame::Body { chunk, .. } => {
                if self.shaken {
                    if let Some(chunk) = chunk {
                        if let Some(base) = chunk.base {
                            try!(base.to_byte_buf(buf));
                        }
                    }
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
    use frame::{WebSocketFrame, FrameCodec};
    use frame::base::{BaseFrame, OpCode};
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
            FrameCodec {
                shaken: true,
                fragmented: true,
                stdout: None,
                stderr: None,
            }
        } else {
            FrameCodec {
                shaken: true,
                fragmented: false,
                stdout: None,
                stderr: None,
            }
        };
        match fc.decode(&mut eb) {
            Ok(Some(decoded)) => {
                match decoded {
                    Frame::Message { message, body } => {
                        if let Some(base) = message.base {
                            assert!(!body);
                            assert!(base.fin());
                            assert!(!base.rsv1());
                            assert!(!base.rsv2());
                            assert!(!base.rsv3());
                            assert!(base.opcode() == opcode);
                            assert!(base.masked() == masked);
                            assert!(base.payload_length() == len);
                            assert!(base.mask_key() == mask);
                            assert!(base.extension_data().is_none());
                            assert!(base.application_data().is_some());
                        } else {
                            assert!(false);
                        }
                    }
                    Frame::Body { chunk, .. } => {
                        if let Some(chunk) = chunk {
                            if let Some(base) = chunk.base {
                                assert!(!base.fin());
                                assert!(base.opcode() == opcode);
                            } else {
                                assert!(false);
                            }
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
        let mut fc = FrameCodec {
            shaken: true,
            fragmented: false,
            stdout: None,
            stderr: None,
        };
        let mut frame: WebSocketFrame = Default::default();
        let mut base: BaseFrame = Default::default();
        base.set_opcode(opcode);
        if opcode == OpCode::Continue {
            base.set_fin(false);
        }
        base.set_payload_length(len);
        base.set_masked(masked);
        base.set_mask_key(mask);
        base.set_application_data(app_data);
        frame.base = Some(base);

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

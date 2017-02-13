//! The `Twist` codec implementation.
//! This is where the decoding/encoding of websocket frames happens.
use frame::{WebSocket, base, handshake};
use slog::Logger;
use std::io;
use tokio_core::io::{Codec, EasyBuf};

/// The `Codec` struct.
pub struct Twist {
    /// The handshake indicator.  If this is false, the handshake is not complete.
    shaken: bool,
    /// An optional stdout slog `Logger`
    stdout: Option<Logger>,
    /// An optional stderr slog `Logger`
    stderr: Option<Logger>,
}

impl Twist {
    /// Add a stdout slog `Logger` to this `Codec`
    pub fn add_stdout(&mut self, stdout: Logger) -> &mut Twist {
        let fc_stdout = stdout.new(o!("module" => module_path!()));
        self.stdout = Some(fc_stdout);
        self
    }

    /// Add a stderr slog `Logger` to this `Codec`
    pub fn add_stderr(&mut self, stderr: Logger) -> &mut Twist {
        let fc_stderr = stderr.new(o!("module" => module_path!()));
        self.stderr = Some(fc_stderr);
        self
    }
}

impl Default for Twist {
    fn default() -> Twist {
        Twist {
            shaken: false,
            stdout: None,
            stderr: None,
        }
    }
}

impl Codec for Twist {
    type In = WebSocket;
    type Out = WebSocket;

    fn decode(&mut self, buf: &mut EasyBuf) -> Result<Option<Self::In>, io::Error> {
        if buf.len() == 0 {
            return Ok(None);
        }

        let mut ws_frame: WebSocket = Default::default();

        if self.shaken {
            let mut base: base::Frame = Default::default();
            if let Ok(Some(base)) = base.decode(buf) {
                ws_frame.set_base(base.clone());
                Ok(Some(ws_frame))
            } else {
                Ok(None)
            }
        } else {
            let mut handshake: handshake::Frame = Default::default();
            match handshake.decode(buf) {
                Ok(Some(hand)) => {
                    ws_frame.set_handshake(hand.clone());
                    Ok(Some(ws_frame))
                }
                Ok(None) => Ok(None),
                Err(e) => {
                    if let Some(ref stderr) = self.stderr {
                        error!(stderr, "{}", e);
                    }
                    Err(e)
                }
            }
        }
    }

    fn encode(&mut self, msg: Self::Out, buf: &mut Vec<u8>) -> io::Result<()> {
        if self.shaken {
            if let Some(base) = msg.base() {
                try!(base.to_byte_buf(buf));
            }
        } else if let Some(handshake) = msg.handshake() {
            try!(handshake.to_byte_buf(buf));
            self.shaken = true;
        } else {
            // TODO: This is probably an error condition.
        }

        Ok(())
    }
}



#[cfg(test)]
mod test {
    use super::Twist;
    use frame::WebSocket;
    use frame::base::{Frame, OpCode};
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
        let mut fc = Twist {
            shaken: true,
            stdout: None,
            stderr: None,
        };

        match fc.decode(&mut eb) {
            Ok(Some(decoded)) => {
                if let Some(base) = decoded.base() {
                    if base.opcode() == OpCode::Continue {
                        assert!(!base.fin());
                    } else {
                        assert!(base.fin());
                    }
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
            Err(_) => {
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
        let mut fc = Twist {
            shaken: true,
            stdout: None,
            stderr: None,
        };
        let mut frame: WebSocket = Default::default();
        let mut base: Frame = Default::default();
        base.set_opcode(opcode);
        if opcode == OpCode::Continue {
            base.set_fin(false);
        }
        base.set_payload_length(len);
        base.set_masked(masked);
        base.set_mask_key(mask);
        base.set_application_data(app_data);
        frame.set_base(base);

        let mut buf = vec![];
        if let Ok(()) = <Twist as Codec>::encode(&mut fc, frame, &mut buf) {
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
        let mut long = Vec::with_capacity(65550);
        long.extend(&[0x88, 0xFF, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00,
                      0x00, 0x01]);
        long.extend([0; 65536].iter());
        decode_test(long, OpCode::Close, true, 65536, Some(1));
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
        let mut long = Vec::with_capacity(65550);
        long.extend(&[0x88, 0xFF, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00,
                      0x00, 0x01]);
        long.extend([0; 65536].iter());
        encode_test(long.to_vec(),
                    OpCode::Close,
                    65536,
                    true,
                    Some(1),
                    Some(vec![0; 65536]));
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

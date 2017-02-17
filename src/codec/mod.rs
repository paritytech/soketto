//! Codec for use with the `WebSocketProtocol`.  Used when decoding/encoding of both websocket
//! handshakes and websocket base frames.
use frame::WebSocket;
use slog::Logger;
use std::io;
use tokio_core::io::{Codec, EasyBuf};

pub mod base;
pub mod handshake;

/// Codec for use with the [`WebSocketProtocol`](struct.WebSocketProtocol.html).  Used when
/// decoding/encoding of both websocket handshakes and websocket base frames.
#[derive(Default)]
pub struct Twist {
    /// An optional stdout slog `Logger`
    stdout: Option<Logger>,
    /// An optional stderr slog `Logger`
    stderr: Option<Logger>,
    /// The handshake indicator.  If this is false, the handshake is not complete.
    shaken: bool,
    /// Is the deflate extension enabled?
    deflate: bool,
    /// blah
    frame_codec: Option<base::FrameCodec>,
    /// blah
    handshake_codec: Option<handshake::FrameCodec>,
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

impl Codec for Twist {
    type In = WebSocket;
    type Out = WebSocket;

    fn decode(&mut self, buf: &mut EasyBuf) -> Result<Option<Self::In>, io::Error> {
        if buf.len() == 0 {
            return Ok(None);
        }

        let mut ws_frame: WebSocket = Default::default();
        if self.shaken {
            if self.frame_codec.is_none() {
                self.frame_codec = Some(Default::default());
            }

            if let Some(ref mut fc) = self.frame_codec {
                match fc.decode(buf) {
                    Ok(Some(frame)) => {
                        ws_frame.set_base(frame);
                    }
                    Ok(None) => return Ok(None),
                    Err(e) => return Err(e),
                }
            }
            self.frame_codec = None;
        } else {
            if self.handshake_codec.is_none() {
                self.handshake_codec = Some(Default::default());
            }
            if let Some(ref mut hc) = self.handshake_codec {
                match hc.decode(buf) {
                    Ok(Some(hand)) => {
                        let extensions = hand.extensions();
                        self.deflate = extensions.contains("permessage-deflate");
                        ws_frame.set_handshake(hand);
                    }
                    Ok(None) => return Ok(None),
                    Err(e) => return Err(e),
                }
            }
            self.handshake_codec = None;
        }
        Ok(Some(ws_frame))
    }

    fn encode(&mut self, msg: Self::Out, buf: &mut Vec<u8>) -> io::Result<()> {
        if self.shaken {
            if let Some(base) = msg.base() {
                let mut fc: base::FrameCodec = Default::default();
                try!(fc.encode(base.clone(), buf));
            }
        } else if let Some(handshake) = msg.handshake() {
            let mut hc: handshake::FrameCodec = Default::default();
            try!(hc.encode(handshake.clone(), buf));
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
    use std::io::{self, Write};
    use tokio_core::io::{Codec, EasyBuf};
    use util;

    #[cfg_attr(rustfmt, rustfmt_skip)]
    const SHORT:  [u8; 7]   = [0x81, 0x81, 0x00, 0x00, 0x00, 0x01, 0x00];
    #[cfg_attr(rustfmt, rustfmt_skip)]
    const MID:    [u8; 134] = [0x81, 0xFE, 0x00, 0x7E, 0x00, 0x00, 0x00, 0x01,
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

    fn decode_test(vec: Vec<u8>, opcode: OpCode, len: u64) {
        let mut eb = EasyBuf::from(vec);
        let mut fc: Twist = Default::default();
        fc.shaken = true;

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
                    assert!(base.payload_length() == len);
                    assert!(base.extension_data().is_none());
                    assert!(base.application_data().is_some());
                } else {
                    assert!(false);
                }
            }
            Err(e) => {
                writeln!(io::stderr(), "{}", e).expect("Unable to write to stderr!");
                assert!(false);
            }
            _ => {
                assert!(false);
            }
        }
    }

    fn encode_test(cmp: Vec<u8>, opcode: OpCode, len: u64, app_data: Option<Vec<u8>>) {
        let mut fc: Twist = Default::default();
        fc.shaken = true;
        let mut frame: WebSocket = Default::default();
        let mut base: Frame = Default::default();
        base.set_opcode(opcode);
        if opcode == OpCode::Continue {
            base.set_fin(false);
        }
        base.set_payload_length(len);
        base.set_application_data(app_data);
        frame.set_base(base);

        let mut buf = vec![];
        if let Ok(()) = <Twist as Codec>::encode(&mut fc, frame, &mut buf) {
            if buf.len() < 1024 {
                println!("{}", util::as_hex(&buf));
            }
            // There is no mask in encoded frames
            assert!(buf.len() == (cmp.len() - 4));
            // TODO: Fix the comparision.  May have to just define separate encoded bufs.
            // for (a, b) in buf.iter().zip(cmp.iter()) {
            //     assert!(a == b);
            // }
        }
    }

    #[test]
    fn decode() {
        decode_test(SHORT.to_vec(), OpCode::Text, 1);
        decode_test(MID.to_vec(), OpCode::Text, 126);
        let mut long = Vec::with_capacity(65550);
        long.extend(&[0x81, 0xFF, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00,
                      0x00, 0x01]);
        long.extend([0; 65536].iter());
        decode_test(long, OpCode::Text, 65536);
        decode_test(CONT.to_vec(), OpCode::Continue, 1);
        decode_test(TEXT.to_vec(), OpCode::Text, 1);
        decode_test(BINARY.to_vec(), OpCode::Binary, 1);
        decode_test(PING.to_vec(), OpCode::Ping, 1);
        decode_test(PONG.to_vec(), OpCode::Pong, 1);
    }

    #[test]
    fn encode() {
        encode_test(SHORT.to_vec(), OpCode::Text, 1, Some(vec![0]));
        encode_test(MID.to_vec(), OpCode::Text, 126, Some(vec![0; 126]));
        let mut long = Vec::with_capacity(65550);
        long.extend(&[0x81, 0xFF, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00,
                      0x00, 0x01]);
        long.extend([0; 65536].iter());
        encode_test(long.to_vec(), OpCode::Close, 65536, Some(vec![0; 65536]));
        encode_test(CONT.to_vec(), OpCode::Continue, 1, Some(vec![0]));
        encode_test(TEXT.to_vec(), OpCode::Text, 1, Some(vec![0]));
        encode_test(BINARY.to_vec(), OpCode::Binary, 1, Some(vec![0]));
        encode_test(PING.to_vec(), OpCode::Ping, 1, Some(vec![0]));
        encode_test(PONG.to_vec(), OpCode::Pong, 1, Some(vec![0]));
    }
}

//! Codec for use with the `WebSocketProtocol`.
//!
//! Used when decoding/encoding of both websocket handshakes and websocket
//! base frames on the server side.

use bytes::BytesMut;
use crate::codec::base::FrameCodec;
use crate::extension::{PerFrameExtensions, PerMessageExtensions};
use crate::frame::WebSocket;
use crate::frame::base::{Frame, OpCode};
use crate::frame::client::request::Frame as ClientHandshakeRequestFrame;
use crate::frame::server::response::Frame as ServerHandshakeResponseFrame;
use crate::util;
use log::{error, trace};
use std::io;
use tokio_io::codec::{Decoder, Encoder};
use uuid::Uuid;
use vatfluid::{Success, validate};

mod http;

pub mod base;
pub mod client;
pub mod server;

/// Codec for use with the [`WebSocketProtocol`].
///
/// Used when decoding/encoding of both websocket handshakes and websocket base frames.
#[derive(Default)]
pub struct Twist {
    /// The Uuid of the parent protocol.  Used for extension lookup.
    uuid: Uuid,
    /// Client/Server flag.
    is_client: bool,
    /// The handshake indicator.  If this is false, the handshake is not complete.
    handshake_complete: bool,
    /// The `Codec` to use to decode the buffer into a `base::Frame`.  Due to the reentrant nature
    /// of decode, the codec is initialized if set to None, reused if Some, and reset to None
    /// after every successful decode.
    frame_codec: Option<FrameCodec>,
    /// The `Codec` use to decode the buffer into a `handshake::Frame`.  Due to the reentrant
    /// nature of decode, the codec is initialized if set to None, reused if Some, and reset to
    /// None after every successful decode.
    client_handshake_codec: Option<client::handshake::FrameCodec>,
    /// The `Codec` use to decode the buffer into a `handshake::Frame`.  Due to the reentrant
    /// nature of decode, the codec is initialized if set to None, reused if Some, and reset to
    /// None after every successful decode.
    server_handshake_codec: Option<server::handshake::FrameCodec>,
    /// The `Origin` header, if present.
    // origin: Option<String>,
    /// Per-message extensions
    permessage_extensions: Option<PerMessageExtensions>,
    /// Per-frame extensions
    perframe_extensions: Option<PerFrameExtensions>,
    /// RSVx bits reserved by extensions (must be less than 16)
    reserved_bits: u8
}

impl Twist {
    /// Create a new `Twist` codec with the given extensions.
    pub fn new(uuid: Uuid, is_client: bool) -> Self {
        Twist {
            uuid,
            is_client,
            handshake_complete: false,
            frame_codec: None,
            client_handshake_codec: None,
            server_handshake_codec: None,
            // origin: None,
            permessage_extensions: None,
            perframe_extensions: None,
            reserved_bits: 0
        }
    }

    /// Run the extension chain decode on the given `base::Frame`.
    fn ext_chain_decode(&self, frame: &mut Frame) -> Result<(), io::Error> {
        let opcode = frame.opcode();
        // Only run the chain if this is a Text/Binary finish frame.
        if frame.fin() && (opcode == OpCode::Text || opcode == OpCode::Binary) {
            if let Some(ref extensions) = self.permessage_extensions {
                let mut map = extensions.lock();
                let vec_pm_exts = map.entry(self.uuid).or_insert_with(Vec::new);
                for ext in vec_pm_exts.iter_mut() {
                    if ext.enabled() {
                        ext.decode(frame)?;
                    }
                }
            }
        }
        Ok(())
    }

    /// Encode a base frame.
    fn encode_base(&mut self, base: &Frame, buf: &mut BytesMut) -> io::Result<()> {
        let mut fc: FrameCodec = Default::default();
        fc.set_client(self.is_client);
        let mut mut_base = base.clone();

        // Run the frame through the permessage extension chain before final encoding.
        if let Some(ref extensions) = self.permessage_extensions {
            let mut map = extensions.lock();
            let vec_pm_exts = map.entry(self.uuid).or_insert_with(Vec::new);
            for ext in vec_pm_exts.iter_mut() {
                if ext.enabled() {
                    ext.encode(&mut mut_base)?;
                }
            }
        }

        fc.encode(mut_base, buf)?;
        Ok(())
    }

    /// Encode a client handshake frame
    fn encode_client_handshake(&mut self,
                               handshake: &ClientHandshakeRequestFrame,
                               buf: &mut BytesMut)
                               -> io::Result<()> {
        let mut hc: client::handshake::FrameCodec = Default::default();
        // Run the frame through the permessage extension chain before final encoding.
        if let Some(ref extensions) = self.permessage_extensions {
            let mut map = extensions.lock();
            let vec_pm_exts = map.entry(self.uuid).or_insert_with(Vec::new);
            for ext in vec_pm_exts.iter_mut() {
                if ext.enabled() {
                    if let Ok(Some(header)) = ext.into_header() {
                        hc.add_header(header);
                    }
                }
            }
        }

        hc.encode(handshake.clone(), buf)?;
        Ok(())
    }

    /// Encode a server handshake frame.
    fn encode_server_handshake(&mut self,
                               handshake: &ServerHandshakeResponseFrame,
                               buf: &mut BytesMut)
                               -> io::Result<()> {
        let mut hc: server::handshake::FrameCodec = Default::default();
        let ext_header = handshake.extensions();
        let mut ext_resp = String::new();
        let mut rb = self.reserved_bits;

        // Run the frame through the permessage extension chain before final encoding.
        if let Some(ref extensions) = self.permessage_extensions {
            let mut map = extensions.lock();
            let vec_pm_exts = map.entry(self.uuid).or_insert_with(Vec::new);
            for ext in vec_pm_exts.iter_mut() {
                ext.from_header(&ext_header)?;
                if ext.enabled() {
                    match ext.reserve_rsv(rb) {
                        Ok(r) => rb = r,
                        Err(e) => return Err(e),
                    }
                    if let Ok(Some(response)) = ext.into_header() {
                        ext_resp.push_str(&response);
                        ext_resp.push_str(", ");
                    }
                }
            }
        }

        self.reserved_bits = rb;
        hc.set_ext_resp(ext_resp.trim_end_matches(", "));

        // TODO: Run through perframe extensions here.

        hc.encode(handshake.clone(), buf)?;
        self.handshake_complete = true;
        Ok(())
    }
}

impl Decoder for Twist {
    type Item = WebSocket;
    type Error = io::Error;

    fn decode(&mut self, buf: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if buf.is_empty() {
            return Ok(None);
        }

        if self.handshake_complete {
            if self.frame_codec.is_none() {
                self.frame_codec = Some(Default::default());
            }

            let mut frame = if let Some(ref mut fc) = self.frame_codec {
                fc.set_client(self.is_client);
                fc.set_reserved_bits(self.reserved_bits);
                match fc.decode(buf) {
                    Ok(Some(frame)) => frame,
                    Ok(None) => return Ok(None),
                    Err(e) => return Err(e),
                }
            } else {
                return Err(util::other("unable to extract frame codec"));
            };

            self.ext_chain_decode(&mut frame)?;

            // Validate utf-8 here to allow pre-processing of appdata by extension chain.
            if frame.opcode() == OpCode::Text && frame.fin() &&
               !frame.application_data().is_empty() {
                match validate(frame.application_data()) {
                    Ok(Success::Complete(pos)) => {
                        trace!("complete: {}", pos);
                    }
                    Ok(Success::Incomplete(_, pos)) => {
                        error!("incomplete: {}", pos);
                        return Err(util::other("invalid utf-8 sequence"));
                    }
                    Err(e) => {
                        error!("{}", e);
                        return Err(util::other("invalid utf-8 sequence"));
                    }
                }
            }

            self.frame_codec = None;
            return Ok(Some(WebSocket::Base(frame)))
        }

        if self.is_client {
            trace!("decoding into server handshake response frame");
            if self.client_handshake_codec.is_none() {
                let hc: client::handshake::FrameCodec = Default::default();
                self.client_handshake_codec = Some(hc);
            }
            if let Some(ref mut hc) = self.client_handshake_codec {
                match hc.decode(buf) {
                    Ok(Some(hand)) => {
                        let ext_header = hand.extensions();
                        let mut rb = self.reserved_bits;

                        // Run the frame through the permessage extension chain..
                        if let Some(ref extensions) = self.permessage_extensions {
                            let mut map = extensions.lock();
                            let vec_pm_exts = map.entry(self.uuid).or_insert_with(Vec::new);
                            for ext in vec_pm_exts.iter_mut() {
                                // Reconfigure based on response
                                ext.from_header(&ext_header)?;

                                // If ext is still enabled set the reserved bits.
                                if ext.enabled() {
                                    match ext.reserve_rsv(rb) {
                                        Ok(r) => rb = r,
                                        Err(e) => return Err(e),
                                    }
                                }
                            }
                        }

                        self.reserved_bits = rb;
                        self.handshake_complete = true;
                        self.client_handshake_codec = None;

                        return Ok(Some(WebSocket::ClientHandshakeResponse(hand)))
                    }
                    Ok(None) => return Ok(None),
                    Err(e) => return Err(e)
                }
            }
        }

        assert!(!self.is_client);

        trace!("decoding into server handshake frame");
        if self.server_handshake_codec.is_none() {
            self.server_handshake_codec = Some(Default::default());
        }

        let hc = self.server_handshake_codec.as_mut().unwrap();
        match hc.decode(buf) {
            Ok(Some(hand)) => {
                self.server_handshake_codec = None;
                return Ok(Some(WebSocket::ServerHandshake(hand)))
            }
            Ok(None) => return Ok(None),
            Err(e) => return Err(e)
        }
    }
}

impl Encoder for Twist {
    type Item = WebSocket;
    type Error = io::Error;

    fn encode(&mut self, msg: Self::Item, buf: &mut BytesMut) -> io::Result<()> {
        trace!("encode: {}", self.is_client);
        match msg {
            WebSocket::Base(ref frame) =>
                if self.handshake_complete {
                    self.encode_base(frame, buf)
                } else {
                    Err(util::other("handshake request not complete"))
                }
            WebSocket::ServerHandshakeResponse(ref frame) => self.encode_server_handshake(frame, buf),
            WebSocket::ClientHandshake(ref frame) => self.encode_client_handshake(frame, buf),
            _ => Err(util::other("unable to extract frame to encode"))
        }
    }
}

#[cfg(test)]
mod test {
    use super::Twist;
    use bytes::BytesMut;
    use crate::frame::WebSocket;
    use crate::frame::base::{Frame, OpCode};
    use crate::util;
    use std::io::{self, Write};
    use tokio_io::codec::{Decoder, Encoder};

    const SHORT:  [u8; 7]   = [0x81, 0x81, 0x00, 0x00, 0x00, 0x01, 0x00];
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
    const CONT:   [u8; 7]   = [0x00, 0x81, 0x00, 0x00, 0x00, 0x01, 0x00];
    const TEXT:   [u8; 7]   = [0x81, 0x81, 0x00, 0x00, 0x00, 0x01, 0x00];
    const BINARY: [u8; 7]   = [0x82, 0x81, 0x00, 0x00, 0x00, 0x01, 0x00];
    const PING:   [u8; 7]   = [0x89, 0x81, 0x00, 0x00, 0x00, 0x01, 0x00];
    const PONG:   [u8; 7]   = [0x8A, 0x81, 0x00, 0x00, 0x00, 0x01, 0x00];

    fn decode_test(vec: Vec<u8>, opcode: OpCode, len: u64) {
        let mut eb = BytesMut::with_capacity(256);
        eb.extend(vec);
        let mut fc: Twist = Default::default();
        fc.handshake_complete = true;

        match fc.decode(&mut eb) {
            Ok(Some(decoded)) => {
                if let WebSocket::Base(base) = decoded {
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
                    assert!(!base.application_data().is_empty());
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

    fn encode_test(cmp: Vec<u8>, opcode: OpCode, len: u64, app_data: Vec<u8>) {
        let mut fc: Twist = Default::default();
        fc.handshake_complete = true;
        let mut base: Frame = Default::default();
        base.set_opcode(opcode);
        if opcode == OpCode::Continue {
            base.set_fin(false);
        }
        base.set_payload_length(len);
        base.set_application_data(app_data);

        let mut buf = BytesMut::with_capacity(1024);
        if let Ok(()) = <Twist as Encoder>::encode(&mut fc, WebSocket::Base(base), &mut buf) {
            if buf.len() < 1024 {
                println!("{}", util::as_hex(&buf));
            }
            // There is no mask in encoded frames
            println!("b: {}, c: {}", buf.len(), cmp.len());
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
        encode_test(SHORT.to_vec(), OpCode::Text, 1, vec![0]);
        encode_test(MID.to_vec(), OpCode::Text, 126, vec![0; 126]);
        let mut long = Vec::with_capacity(65550);
        long.extend(&[0x81, 0xFF, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00,
                      0x00, 0x01]);
        long.extend([0; 65536].iter());
        encode_test(long.to_vec(), OpCode::Close, 65536, vec![0; 65536]);
        encode_test(CONT.to_vec(), OpCode::Continue, 1, vec![0]);
        encode_test(TEXT.to_vec(), OpCode::Text, 1, vec![0]);
        encode_test(BINARY.to_vec(), OpCode::Binary, 1, vec![0]);
        encode_test(PING.to_vec(), OpCode::Ping, 1, vec![0]);
        encode_test(PONG.to_vec(), OpCode::Pong, 1, vec![0]);
    }
}

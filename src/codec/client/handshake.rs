//! Codec for dedoding/encoding websocket client handshake frames.
use frame::client::handshake::Frame;
use slog::Logger;
use std::io;
use tokio_core::io::{Codec, EasyBuf};

/// Codec for decoding/encoding websocket client handshake frames.
#[derive(Default)]
pub struct FrameCodec {
    /// slog stdout `Logger`
    stdout: Option<Logger>,
    /// slog stderr `Logger`
    stderr: Option<Logger>,
}

impl FrameCodec {
    /// Add a stdout slog `Logger` to this protocol.
    pub fn stdout(&mut self, logger: Logger) -> &mut FrameCodec {
        let stdout = logger.new(o!("codec" => "client::handshake"));
        self.stdout = Some(stdout);
        self
    }

    /// Add a stderr slog `Logger` to this protocol.
    pub fn stderr(&mut self, logger: Logger) -> &mut FrameCodec {
        let stderr = logger.new(o!("codec" => "client::handshake"));
        self.stderr = Some(stderr);
        self
    }
}

impl Codec for FrameCodec {
    type In = Frame;
    type Out = Frame;

    fn decode(&mut self, buf: &mut EasyBuf) -> Result<Option<Self::In>, io::Error> {
        let len = buf.len();
        try_trace!(self.stdout,
                   "server handshake response length: {}",
                   buf.len());
        if buf.len() == 0 {
            return Ok(None);
        }
        try_trace!(self.stdout,
                   "server handshake response\n{}",
                   String::from_utf8_lossy(buf.as_slice()));
        buf.drain_to(len);
        let frame = Frame::new()?;
        Ok(Some(frame))
    }

    fn encode(&mut self, msg: Self::Out, buf: &mut Vec<u8>) -> io::Result<()> {
        let mut request = String::from("GET / HTTP/1.1\r\n");
        request.push_str(&format!("User-Agent: {}\r\n", msg.user_agent()));
        request.push_str(&format!("Host: {}\r\n", msg.host()));
        request.push_str("Upgrade: websocket\r\n");
        request.push_str("Connection: upgrade\r\n");
        request.push_str(&format!("Sec-WebSocket-Key: {}\r\n", msg.sec_websocket_key()));
        request.push_str("Sec-WebSocket-Version: 13\r\n");
        request.push_str("\r\n");

        try_trace!(self.stdout, "client handshake request\n{}", request);
        buf.extend(request.as_bytes());
        Ok(())
    }
}

//! Codec for dedoding/encoding websocket client handshake frames.
use frame::client::request::Frame as ClientRequest;
use frame::client::response::Frame as ServerResponse;
use httparse::{EMPTY_HEADER, Response};
use slog::Logger;
use std::collections::HashMap;
use std::io;
use tokio_core::io::{Codec, EasyBuf};
use util;

/// Codec for decoding/encoding websocket client handshake frames.
#[derive(Default)]
pub struct FrameCodec {
    /// slog stdout `Logger`
    stdout: Option<Logger>,
    /// slog stderr `Logger`
    stderr: Option<Logger>,
    /// The extensions headers to send with the request.
    extension_headers: Vec<String>,
}

impl FrameCodec {
    /// Add a `Sec-WebSocket-Extensions` header to this client handshake.
    pub fn add_header(&mut self, header: String) -> &mut FrameCodec {
        self.extension_headers.push(header);
        self
    }

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
    type In = ServerResponse;
    type Out = ClientRequest;

    fn decode(&mut self, buf: &mut EasyBuf) -> Result<Option<Self::In>, io::Error> {
        let len = buf.len();
        if len == 0 {
            return Ok(None);
        }
        let drained = buf.drain_to(len);
        let resp_bytes = drained.as_slice();
        let mut headers = [EMPTY_HEADER; 32];
        let mut resp = Response::new(&mut headers);
        let mut handshake_frame: ServerResponse = Default::default();

        if let Ok(res) = resp.parse(resp_bytes) {
            if res.is_complete() {
                if let Some(version) = resp.version {
                    handshake_frame.set_version(version);
                }

                if let Some(code) = resp.code {
                    handshake_frame.set_code(code);
                }

                if let Some(reason) = resp.reason {
                    handshake_frame.set_reason(reason);
                }

                let mut headers = HashMap::new();
                for header in resp.headers {
                    // Duplicate headers are concatenated as comma-separated string.
                    let key = header.name.to_string();
                    let val = String::from_utf8_lossy(header.value).into_owned();
                    let mut entry = headers.entry(key).or_insert_with(String::new);

                    if entry.is_empty() {
                        entry.push_str(&val);
                    } else {
                        entry.push(',');
                        entry.push_str(&val);
                    }
                }

                // Required Headers
                handshake_frame.set_upgrade(headers.remove("Upgrade"));
                handshake_frame.set_conn(headers.remove("Connection"));
                handshake_frame.set_ws_accept(headers.remove("Sec-WebSocket-Accept"));

                // Optional headers
                handshake_frame.set_protocol(headers.remove("Sec-WebSocket-Protocol"));
                handshake_frame.set_extensions(headers.remove("Sec-WebSocket-Extensions"));

                if !headers.is_empty() {
                    handshake_frame.set_others(headers);
                }

                if handshake_frame.validate() {
                    Ok(Some(handshake_frame))
                } else {
                    Err(util::other("invalid handshake request"))
                }
            } else {
                return Ok(None);
            }
        } else {
            return Err(util::other("unable to parse client request"));
        }
    }

    fn encode(&mut self, msg: Self::Out, buf: &mut Vec<u8>) -> io::Result<()> {
        let mut request = String::from("GET / HTTP/1.1\r\n");
        request.push_str(&format!("User-Agent: {}\r\n", msg.user_agent()));
        request.push_str(&format!("Host: {}\r\n", msg.host()));
        request.push_str("Upgrade: websocket\r\n");
        request.push_str("Connection: upgrade\r\n");
        request.push_str(&format!("Sec-WebSocket-Key: {}\r\n", msg.sec_websocket_key()));
        request.push_str("Sec-WebSocket-Version: 13\r\n");

        for header in &self.extension_headers {
            request.push_str(header);
            request.push_str("\r\n");
        }

        request.push_str("\r\n");

        try_trace!(self.stdout, "client handshake request\n{}", request);
        buf.extend(request.as_bytes());
        Ok(())
    }
}

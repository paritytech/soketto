//! Server response frame to a client request

use crate::util::Invalid;
use sha1::Sha1;
use std::borrow::Cow;

/// Defined in RFC6455 and used to generate the `Sec-WebSocket-Accept` header in the server
/// handshake response.
const KEY: &[u8] = b"258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

#[derive(Debug)]
pub struct Builder(http::response::Builder);

#[derive(Debug)]
pub struct ServerHandshake(http::Response<()>);

impl Builder {
    pub fn accept(key: &str) -> Self {
        let accept = {
            let mut digest = Sha1::new();
            digest.update(key.as_bytes());
            digest.update(KEY);
            base64::encode(&digest.digest().bytes())
        };

        let mut rb = http::Response::builder();
        rb.status(http::StatusCode::SWITCHING_PROTOCOLS)
            .version(http::Version::HTTP_11)
            .header(http::header::UPGRADE, "websocket")
            .header(http::header::CONNECTION, http::header::UPGRADE)
            .header(http::header::SEC_WEBSOCKET_KEY, key)
            .header(http::header::SEC_WEBSOCKET_ACCEPT, accept);

        Self(rb)
    }

    pub fn protocol(&mut self, proto: &str) -> &mut Self {
        self.0.header(http::header::SEC_WEBSOCKET_PROTOCOL, proto);
        self
    }

    pub fn finish<'a>(mut self) -> Result<ServerHandshake, Invalid<'a>> {
        self.0.body(())
            .map_err(|_| Invalid(Cow::Borrowed("invalid 'Response' construction")))
            .map(ServerHandshake)
    }
}

impl ServerHandshake {
    pub fn response(&self) -> &http::Response<()> {
        &self.0
    }
}

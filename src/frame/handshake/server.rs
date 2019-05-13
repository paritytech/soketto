use crate::Nonce;
use sha1::Sha1;
use super::{expect_header, with_header, Invalid};

/// Websocket handshake response builder.
#[derive(Debug)]
pub struct Builder {
    response: http::response::Builder,
    ws_key: Nonce
}

impl Builder {
    pub fn add_protocol(&mut self, proto: &str) -> &mut Self {
        self.response.header(http::header::SEC_WEBSOCKET_PROTOCOL, proto);
        self
    }

    pub fn add_extension(&mut self, ext: &str) -> &mut Self {
        self.response.header(http::header::SEC_WEBSOCKET_EXTENSIONS, ext);
        self
    }

    pub fn finish(mut self) -> Result<Response, Invalid> {
        let r = self.response.body(()).map_err(|e| Invalid::new(format!("{}", e)))?;
        Ok(Response { response: r, ws_key: self.ws_key })
    }
}

/// The server's websocket handshake response.
#[derive(Debug)]
pub struct Response {
    response: http::Response<()>,
    ws_key: Nonce
}

impl Response {
    pub fn accept(ws_key: Nonce) -> Builder {
        let accept = {
            let mut digest = Sha1::new();
            digest.update(ws_key.as_ref().as_bytes());
            digest.update(super::KEY);
            base64::encode(&digest.digest().bytes())
        };

        let mut rb = http::Response::builder();
        rb.status(http::StatusCode::SWITCHING_PROTOCOLS)
            .version(http::Version::HTTP_11)
            .header(http::header::UPGRADE, "websocket")
            .header(http::header::CONNECTION, http::header::UPGRADE)
            .header(http::header::SEC_WEBSOCKET_KEY, ws_key.as_ref())
            .header(http::header::SEC_WEBSOCKET_ACCEPT, accept);

        Builder { response: rb, ws_key }
    }

    // TODO: check protocol is one of the ones requested.
    pub(crate) fn new(ws_key: Nonce, response: http::Response<()>) -> Result<Self, Invalid> {
        if response.version() != http::Version::HTTP_11 {
            return Err(Invalid::new("unsupported HTTP version"))
        }

        if response.status() != http::StatusCode::SWITCHING_PROTOCOLS {
            return Err(Invalid::new("unexpected HTTP status code"))
        }

        expect_header(response.headers(), &http::header::UPGRADE, "websocket")?;
        expect_header(response.headers(), &http::header::CONNECTION, "upgrade")?;

        let nonce = ws_key.as_ref().as_bytes();
        with_header(response.headers(), &http::header::SEC_WEBSOCKET_ACCEPT, move |theirs| {
            let mut digest = Sha1::new();
            digest.update(nonce);
            digest.update(super::KEY);
            let ours = base64::encode(&digest.digest().bytes());
            if ours != theirs {
                return Err(Invalid::new("invalid 'Sec-WebSocket-Accept' received"))
            }
            Ok(())
        })?;

        Ok(Response { response, ws_key })
    }

    pub fn as_http(&self) -> &http::Response<()> {
        &self.response
    }

    pub fn websocket_key(&self) -> &Nonce {
        &self.ws_key
    }

    pub fn websocket_extensions(&self) -> impl Iterator<Item = &http::header::HeaderValue> {
        self.response
            .headers()
            .get_all(&http::header::SEC_WEBSOCKET_EXTENSIONS)
            .into_iter()
    }

    pub fn websocket_protocol(&self) -> Option<&http::header::HeaderValue> {
        self.response.headers().get(&http::header::SEC_WEBSOCKET_PROTOCOL)
    }
}


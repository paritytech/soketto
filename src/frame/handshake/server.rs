use crate::util::{self, Invalid, Nonce};
use sha1::Sha1;
use std::borrow::Cow;

#[derive(Debug)]
pub struct ResponseBuilder(http::response::Builder);

/// The server's handshake response.
#[derive(Debug)]
pub struct Response(http::Response<()>);

impl ResponseBuilder {
    pub fn accept(nonce: &Nonce) -> Self {
        let accept = {
            let mut digest = Sha1::new();
            digest.update(nonce.as_ref().as_bytes());
            digest.update(util::KEY);
            base64::encode(&digest.digest().bytes())
        };

        let mut rb = http::Response::builder();
        rb.status(http::StatusCode::SWITCHING_PROTOCOLS)
            .version(http::Version::HTTP_11)
            .header(http::header::UPGRADE, "websocket")
            .header(http::header::CONNECTION, http::header::UPGRADE)
            .header(http::header::SEC_WEBSOCKET_KEY, nonce.as_ref())
            .header(http::header::SEC_WEBSOCKET_ACCEPT, accept);

        Self(rb)
    }

    pub fn protocol(&mut self, proto: &str) -> &mut Self {
        self.0.header(http::header::SEC_WEBSOCKET_PROTOCOL, proto);
        self
    }

    pub fn finish<'a>(mut self) -> Result<Response, Invalid<'a>> {
        self.0.body(())
            .map_err(|_| Invalid(Cow::Borrowed("invalid 'Response' construction")))
            .map(Response)
    }
}

impl Response {
    pub fn as_http(&self) -> &http::Response<()> {
        &self.0
    }
}


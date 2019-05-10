//! Client handshake frame received by server.

use std::borrow::Cow;
use crate::util::{with_header, expect_header, Invalid};

#[derive(Debug)]
pub struct ClientHandshake<S = Validated>(S);

#[derive(Debug)]
pub struct Validated {
    request: http::Request<()>,
    ws_key: String
}

impl ClientHandshake<http::Request<()>> {
    pub(crate) fn new(r: http::Request<()>) -> Self {
        Self(r)
    }

    pub fn request(&self) -> &http::Request<()> {
        &self.0
    }

    pub fn validated<'a>(self) -> Result<ClientHandshake<Validated>, Invalid<'a>> {
        if self.request().method() != http::Method::GET {
            return Err(Invalid(Cow::Borrowed("request method != GET")))
        }

        if self.request().version() != http::Version::HTTP_11 {
            return Err(Invalid(Cow::Borrowed("unsupported HTTP version")))
        }

        // TODO: Host Validation

        expect_header(self.request(), &http::header::UPGRADE, "websocket")?;
        expect_header(self.request(), &http::header::CONNECTION, "upgrade")?;
        expect_header(self.request(), &http::header::SEC_WEBSOCKET_VERSION, "13")?;

        let ws_key = with_header(self.request(), &http::header::SEC_WEBSOCKET_KEY, |k| {
            Ok(String::from(k))
        })?;

        Ok(ClientHandshake(Validated { request: self.0, ws_key }))
    }
}

impl ClientHandshake<Validated> {
    pub fn request(&self) -> &http::Request<()> {
        &self.0.request
    }

    pub fn websocket_key(&self) -> &str {
        &self.0.ws_key
    }

    pub fn websocket_extensions(&self) -> impl Iterator<Item = &http::header::HeaderValue> {
        self.request()
            .headers()
            .get_all(&http::header::SEC_WEBSOCKET_EXTENSIONS)
            .into_iter()
    }

    pub fn websocket_protocols(&self) -> impl Iterator<Item = &http::header::HeaderValue> {
        self.request()
            .headers()
            .get_all(&http::header::SEC_WEBSOCKET_PROTOCOL)
            .into_iter()
    }
}


use sha1::Sha1;
use std::borrow::Cow;
use crate::util::{self, with_header, expect_header, Invalid, Nonce};

/// The client's handshake HTTP request.
#[derive(Debug)]
pub struct Request {
    request: http::Request<()>,
    ws_key: Nonce
}

impl Request {
    pub(crate) fn new<'a>(request: http::Request<()>) -> Result<Self, Invalid<'a>> {
        if request.method() != http::Method::GET {
            return Err(Invalid(Cow::Borrowed("request method != GET")))
        }

        if request.version() != http::Version::HTTP_11 {
            return Err(Invalid(Cow::Borrowed("unsupported HTTP version")))
        }

        // TODO: Host Validation
        with_header(request.headers(), &http::header::HOST, |h| Ok(()))?;

        expect_header(request.headers(), &http::header::UPGRADE, "websocket")?;
        expect_header(request.headers(), &http::header::CONNECTION, "upgrade")?;
        expect_header(request.headers(), &http::header::SEC_WEBSOCKET_VERSION, "13")?;

        let ws_key = with_header(request.headers(), &http::header::SEC_WEBSOCKET_KEY, |k| {
            Ok(Nonce::wrap(k.into()))
        })?;

        Ok(Request { request, ws_key })
    }

    pub fn as_http(&self) -> &http::Request<()> {
        &self.request
    }

    pub fn websocket_key(&self) -> &Nonce {
        &self.ws_key
    }

    pub fn websocket_extensions(&self) -> impl Iterator<Item = &http::header::HeaderValue> {
        self.request
            .headers()
            .get_all(&http::header::SEC_WEBSOCKET_EXTENSIONS)
            .into_iter()
    }

    pub fn websocket_protocols(&self) -> impl Iterator<Item = &http::header::HeaderValue> {
        self.request
            .headers()
            .get_all(&http::header::SEC_WEBSOCKET_PROTOCOL)
            .into_iter()
    }
}


/// The server's handshake HTTP response.
#[derive(Debug)]
pub struct Response {
    response: http::Response<()>
}

impl Response {
    // TODO: check extension is one of the onces requested.
    // TODO: check protocol is one of the ones requested.
    pub(crate) fn new<'a>(nonce: &Nonce, response: http::Response<()>) -> Result<Self, Invalid<'a>> {
        if response.version() != http::Version::HTTP_11 {
            return Err(Invalid(Cow::Borrowed("unsupported HTTP version")))
        }

        if response.status() != http::StatusCode::SWITCHING_PROTOCOLS {
            return Err(Invalid(Cow::Borrowed("unexpected HTTP status code")))
        }

        expect_header(response.headers(), &http::header::UPGRADE, "websocket")?;
        expect_header(response.headers(), &http::header::CONNECTION, "upgrade")?;

        with_header(response.headers(), &http::header::SEC_WEBSOCKET_ACCEPT, move |theirs| {
            let mut digest = Sha1::new();
            digest.update(nonce.as_ref().as_bytes());
            digest.update(util::KEY);
            let ours = base64::encode(&digest.digest().bytes());
            if ours != theirs {
                return Err(Invalid(Cow::Borrowed("invalid 'Sec-WebSocket-Accept' received")))
            }
            Ok(())
        })?;

        Ok(Response { response })
    }
}

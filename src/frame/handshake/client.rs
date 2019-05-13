use crate::Nonce;
use super::{expect_header, with_header, Invalid};

/// Websocket handshake request builder.
#[derive(Debug)]
pub struct Builder {
    request: http::request::Builder,
    ws_key: Nonce
}

impl Builder {
    pub fn add_protocol(&mut self, proto: &str) -> &mut Self {
        self.request.header(http::header::SEC_WEBSOCKET_PROTOCOL, proto);
        self
    }

    pub fn add_extension(&mut self, ext: &str) -> &mut Self {
        self.request.header(http::header::SEC_WEBSOCKET_EXTENSIONS, ext);
        self
    }

    pub fn finish(mut self) -> Result<Request, Invalid> {
        let r = self.request.body(()).map_err(|e| Invalid::new(format!("{}", e)))?;
        Ok(Request { request: r, ws_key: self.ws_key })
    }
}

/// The client's websocket handshake request.
#[derive(Debug)]
pub struct Request {
    request: http::Request<()>,
    ws_key: Nonce
}

impl Request {
    pub fn builder(ws_key: Nonce) -> Builder {
        let mut rb = http::Request::builder();
        rb.method(http::Method::GET);
        rb.version(http::Version::HTTP_11);
        rb.header(http::header::UPGRADE, "websocket");
        rb.header(http::header::CONNECTION, "upgrade");
        rb.header(http::header::SEC_WEBSOCKET_VERSION, "13");
        rb.header(http::header::SEC_WEBSOCKET_KEY, ws_key.as_ref());
        Builder { request: rb, ws_key }
    }

    pub(crate) fn new(request: http::Request<()>) -> Result<Self, Invalid> {
        if request.method() != http::Method::GET {
            return Err(Invalid::new("request method != GET"))
        }

        if request.version() != http::Version::HTTP_11 {
            return Err(Invalid::new("unsupported HTTP version"))
        }

        // TODO: Host Validation
        with_header(request.headers(), &http::header::HOST, |_h| Ok(()))?;

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


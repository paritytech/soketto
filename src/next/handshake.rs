use bytes::BytesMut;
use crate::next::error::Error;
use either::Either;
use sha1::Sha1;
use std::{borrow::Borrow, iter, str};
use tokio_io::codec::{Decoder, Encoder};

// Request ////////////////////////////////////////////////////////////////////////////////////////

/// A websocket handshake request.
#[derive(Debug)]
pub struct Request<S, I> {
    path: S,
    key: S,
    origin: Option<S>,
    protocols: Option<I>,
    extensions: Option<I>
}

impl<S, I> Request<S, I>
where
    S: Borrow<str>,
    I: IntoIterator<Item = S> + Clone
{
    pub fn new(path: S, key: S) -> Self {
        Self {
            path,
            key,
            origin: None,
            protocols: None,
            extensions: None
        }
    }

    pub fn path(&self) -> &str {
        self.path.borrow()
    }

    pub fn key(&self) -> &str {
        self.key.borrow()
    }

    pub fn origin(&self) -> Option<&str> {
        self.origin.as_ref().map(|o| o.borrow())
    }

    pub fn set_origin(&mut self, o: S) -> &mut Self {
        self.origin = Some(o);
        self
    }

    pub fn extensions(&self) -> impl Iterator<Item = S> {
        self.extensions
            .clone()
            .map(Either::Left)
            .unwrap_or_else(|| Either::Right(iter::empty()))
            .into_iter()
    }

    pub fn set_extensions(&mut self, ext: I) -> &mut Self {
        self.extensions = Some(ext);
        self
    }

    pub fn protocols(&self) -> impl Iterator<Item = S> {
        self.protocols
            .clone()
            .map(Either::Left)
            .unwrap_or_else(|| Either::Right(iter::empty()))
            .into_iter()
    }

    pub fn set_protocols(&mut self, protos: I) -> &mut Self {
        self.protocols = Some(protos);
        self
    }
}

// Response //////////////////////////////////////////////////////////////////////////////////////

/// A websocket handshake response.
#[derive(Debug)]
pub struct Response<S, I> {
    protocol: Option<S>,
    extensions: Option<I>
}

impl<S, I> Response<S, I>
where
    S: Borrow<str>,
    I: IntoIterator<Item = S> + Clone
{
    pub fn new() -> Self {
        Self {
            protocol: None,
            extensions: None
        }
    }

    pub fn protocol(&self) -> Option<&str> {
        self.protocol.as_ref().map(|o| o.borrow())
    }

    pub fn set_protocol(&mut self, p: S) -> &mut Self {
        self.protocol = Some(p);
        self
    }

    pub fn extensions(&self) -> impl Iterator<Item = S> {
        self.extensions
            .clone()
            .map(Either::Left)
            .unwrap_or_else(|| Either::Right(iter::empty()))
            .into_iter()
    }

    pub fn set_extensions(&mut self, ext: I) -> &mut Self {
        self.extensions = Some(ext);
        self
    }
}

// Handshake codec ///////////////////////////////////////////////////////////////////////////////

// Defined in RFC6455 and used to generate the `Sec-WebSocket-Accept` header
// in the server handshake response.
const KEY: &[u8] = b"258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

// How many HTTP headers do we support during parsing?
const MAX_NUM_HEADERS: usize = 32;

// Some HTTP headers we need to check during parsing.
const SEC_WEBSOCKET_EXTENSIONS: unicase::Ascii<&str> = unicase::Ascii::new("Sec-WebSocket-Extensions");
const SEC_WEBSOCKET_PROTOCOL: unicase::Ascii<&str> = unicase::Ascii::new("Sec-WebSocket-Protocol");

#[derive(Debug)]
pub enum Codec<'a> {
    Client {
        nonce: &'a str,
        protocols: &'a [&'a str],
        extensions: &'a [&'a str]
    },
    Server
}

impl<'a> Codec<'a> {
    pub fn client(nonce: &'a str) -> Self {
        Codec::Client { nonce, protocols: &[], extensions: &[] }
    }

    pub fn server() -> Self {
        Codec::Server
    }
}

//impl<'a, S, I> Encoder for Codec<'a>
//where
//    S: Borrow<str>,
//    I: IntoIterator<Item = S> + Clone
//{
//    type Item = Either<Request<S, I>, Response<S, I>>;
//    type Error = Error;
//
//    fn encode(&mut self, item: Self::Item, buf: &mut BytesMut) -> Result<(), Self::Error> {
//        unimplemented!()
//    }
//}

impl<'a> Decoder for Codec<'a> {
    type Item = Either<Request<String, Vec<String>>, Response<&'a str, Vec<&'a str>>>;
    type Error = Error;

    fn decode(&mut self, bytes: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        let mut header_buf = [httparse::EMPTY_HEADER; MAX_NUM_HEADERS];

        let (result, offset) = match self {
            Codec::Client { nonce, protocols, extensions } => { // decode server response
                let mut response = httparse::Response::new(&mut header_buf);

                let offset = match response.parse(bytes) {
                    Ok(httparse::Status::Complete(off)) => off,
                    Ok(httparse::Status::Partial) => return Ok(None),
                    Err(e) => return Err(Error::Http(Box::new(e)))
                };

                if response.version != Some(1) {
                    return Err(Error::Invalid("unsupported HTTP version".into()))
                }
                if response.code != Some(101) {
                    return Err(Error::Invalid("unexpected HTTP status code".into()))
                }

                expect_header(&response.headers, "Upgrade", "websocket")?;
                expect_header(&response.headers, "Connection", "upgrade")?;

                with_header(&response.headers, "Sec-WebSocket-Accept", move |theirs| {
                    let mut digest = Sha1::new();
                    digest.update(nonce.as_bytes());
                    digest.update(KEY);
                    let ours = base64::encode(&digest.digest().bytes());
                    if ours.as_bytes() != theirs {
                        return Err(Error::Invalid("invalid 'Sec-WebSocket-Accept' received".into()))
                    }
                    Ok(())
                })?;

                let mut result = Response::new();

                // Collect matching `Sec-WebSocket-Extensions` headers.

                let mut selected_extensions = Vec::with_capacity(extensions.len());
                for header in response.headers.iter()
                    .filter(|h| unicase::Ascii::new(h.name) == SEC_WEBSOCKET_EXTENSIONS)
                {
                    match extensions.iter().find(|x| x.as_bytes() == header.value) {
                        Some(&x) => selected_extensions.push(x),
                        None => return Err(Error::Invalid("extension was not requested".into()))
                    }
                }

                result.set_extensions(selected_extensions);

                // Get matching `Sec-WebSocket-Protocol` header.

                let their_proto = response.headers.iter()
                    .find(|h| unicase::Ascii::new(h.name) == SEC_WEBSOCKET_PROTOCOL);

                if let Some(tp) = their_proto {
                    if let Some(&p) = protocols.iter().find(|x| x.as_bytes() == tp.value) {
                        result.set_protocol(p);
                    } else {
                        return Err(Error::Invalid("protocol was not requested".into()))
                    }
                }

                (Either::Right(result), offset)
            }
            Codec::Server => { // decode client request
                let mut request = httparse::Request::new(&mut header_buf);

                let offset = match request.parse(bytes) {
                    Ok(httparse::Status::Complete(off)) => off,
                    Ok(httparse::Status::Partial) => return Ok(None),
                    Err(e) => return Err(Error::Http(Box::new(e)))
                };

                if request.method != Some("GET") {
                    return Err(Error::Invalid("request method != GET".into()))
                }
                if request.version != Some(1) {
                    return Err(Error::Invalid("unsupported HTTP version".into()))
                }

                // TODO: Host Validation
                with_header(&request.headers, "Host", |_h| Ok(()))?;

                expect_header(&request.headers, "Upgrade", "websocket")?;
                expect_header(&request.headers, "Connection", "upgrade")?;
                expect_header(&request.headers, "Sec-WebSocket-Version", "13")?;

                let ws_key = with_header(&request.headers, "Sec-WebSocket-Key", |k| {
                    Ok(String::from(str::from_utf8(k)?))
                })?;

                let path = request.path.unwrap_or("/");
                let mut result = Request::new(String::from(path), ws_key);

                let mut extensions = Vec::new();
                for header in request.headers.iter()
                    .filter(|h| unicase::Ascii::new(h.name) == SEC_WEBSOCKET_EXTENSIONS)
                {
                    extensions.push(str::from_utf8(header.value)?.into())
                }
                result.set_extensions(extensions);

                let mut protocols = Vec::new();
                for header in request.headers.iter()
                    .filter(|h| unicase::Ascii::new(h.name) == SEC_WEBSOCKET_PROTOCOL)
                {
                    protocols.push(str::from_utf8(header.value)?.into())
                }
                result.set_protocols(protocols);

                (Either::Left(result), offset)
            }
        };

        bytes.split_to(offset); // chop off the HTTP part we have processed
        Ok(Some(result))
    }
}

fn expect_header(headers: &[httparse::Header], name: &str, ours: &str) -> Result<(), Error> {
    with_header(headers, name, move |theirs| {
        let s = str::from_utf8(theirs)?;
        if unicase::Ascii::new(s) == unicase::Ascii::new(ours) {
            Ok(())
        } else {
            Err(Error::Invalid(format!("invalid value for header {}", name)))
        }
    })
}

fn with_header<F, R>(headers: &[httparse::Header], name: &str, f: F) -> Result<R, Error>
where
    F: Fn(&[u8]) -> Result<R, Error>
{
    let ascii_name = unicase::Ascii::new(name);
    if let Some(h) = headers.iter().find(move |h| unicase::Ascii::new(h.name) == ascii_name) {
        f(h.value)
    } else {
        Err(Error::Invalid(format!("header {} not found", name)))
    }
}


use bytes::BytesMut;
use std::{fmt, io};
use tokio_io::codec::{Decoder, Encoder};

// Request ////////////////////////////////////////////////////////////////////////////////////////

/// HTTP/1.1 request header encoder/decoder.
#[derive(Debug)]
pub struct RequestHeaderCodec(());

impl RequestHeaderCodec {
    pub fn new() -> Self { Self(()) }
}

impl Decoder for RequestHeaderCodec {
    type Item = http::Request<()>;
    type Error = Error;

    fn decode(&mut self, bytes: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        let mut header_buf = [httparse::EMPTY_HEADER; 64];
        let mut req_parser = httparse::Request::new(&mut header_buf[..]);

        match req_parser.parse(&bytes[..]) {
            Ok(httparse::Status::Complete(_)) => (),
            Ok(httparse::Status::Partial) => return Ok(None),
            Err(e) => return Err(Error::Parse(Box::new(e)))
        }

        let mut rb = http::Request::builder();

        rb.method(req_parser.method.expect("request parsed successfully"))
            .uri(req_parser.path.expect("request parsed successfully"))
            .version(match req_parser.version.expect("response parsed successfully") {
                0 => http::Version::HTTP_10,
                1 => http::Version::HTTP_11,
                n => return Err(Error::UnknownHttpVersion(n))
            });

        for h in req_parser.headers {
            rb.header(h.name, h.value);
        }

        Ok(Some(rb.body(())?))
    }
}

impl Encoder for RequestHeaderCodec {
    type Item = http::Request<()>;
    type Error = Error;

    fn encode(&mut self, rq: Self::Item, buf: &mut BytesMut) -> Result<(), Self::Error> {
        buf.extend_from_slice(rq.method().as_str().as_bytes());
        buf.extend_from_slice(b" ");
        buf.extend_from_slice(rq.uri()
            .path_and_query()
            .map(|p| p.as_str())
            .unwrap_or("/")
            .as_bytes());
        buf.extend_from_slice(b" HTTP/1.1\r\n");

        for (k, v) in rq.headers() {
            buf.extend_from_slice(k.as_str().as_bytes());
            buf.extend_from_slice(b": ");
            buf.extend_from_slice(v.as_bytes());
            buf.extend_from_slice(b"\r\n")
        }

        buf.extend_from_slice(b"\r\n");
        Ok(())
    }
}

// Response ///////////////////////////////////////////////////////////////////////////////////////

/// HTTP/1.1 response header encoder/decoder.
#[derive(Debug)]
pub struct ResponseHeaderCodec<'a>(std::marker::PhantomData<&'a ()>);

impl<'a> ResponseHeaderCodec<'a> {
    pub fn new() -> Self { Self(std::marker::PhantomData) }
}

impl<'a> Decoder for ResponseHeaderCodec<'a> {
    type Item = http::Response<()>;
    type Error = Error;

    fn decode(&mut self, bytes: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        let mut header_buf = [httparse::EMPTY_HEADER; 64];
        let mut res_parser = httparse::Response::new(&mut header_buf[..]);

        match res_parser.parse(&bytes[..]) {
            Ok(httparse::Status::Complete(_)) => (),
            Ok(httparse::Status::Partial) => return Ok(None),
            Err(e) => return Err(Error::Parse(Box::new(e)))
        }

        let mut rb = http::Response::builder();

        rb.status(res_parser.code.expect("response parsed successfully"))
            .version(match res_parser.version.expect("response parsed successfully") {
                0 => http::Version::HTTP_10,
                1 => http::Version::HTTP_11,
                n => return Err(Error::UnknownHttpVersion(n))
            });

        for h in res_parser.headers {
            rb.header(h.name, h.value);
        }

        Ok(Some(rb.body(())?))
    }
}

impl<'a> Encoder for ResponseHeaderCodec<'a> {
    type Item = &'a http::Response<()>;
    type Error = Error;

    fn encode(&mut self, rs: Self::Item, buf: &mut BytesMut) -> Result<(), Self::Error> {
        buf.extend_from_slice(b"HTTP/1.1 ");
        buf.extend_from_slice(rs.status().as_str().as_bytes());
        buf.extend_from_slice(rs.status().canonical_reason().unwrap_or("N/A").as_bytes());
        buf.extend_from_slice(b"\r\n");

        for (k, v) in rs.headers() {
            buf.extend_from_slice(k.as_str().as_bytes());
            buf.extend_from_slice(b": ");
            buf.extend_from_slice(v.as_bytes());
            buf.extend_from_slice(b"\r\n")
        }

        buf.extend_from_slice(b"\r\n");
        Ok(())
    }
}

// Error type /////////////////////////////////////////////////////////////////////////////////////

#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    Http(http::Error),
    Parse(Box<dyn std::error::Error + Send + 'static>),
    UnknownHttpVersion(u8)
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::Io(e) => write!(f, "i/o error: {}", e),
            Error::Http(e) => write!(f, "http error: {}", e),
            Error::Parse(e) => write!(f, "parse error: {}", e),
            Error::UnknownHttpVersion(n) => write!(f, "unknown http version ({})", n)
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io(e) => Some(e),
            Error::Http(e) => Some(e),
            Error::Parse(e) => Some(&**e),
            Error::UnknownHttpVersion(_) => None
        }
    }
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Error::Io(e)
    }
}

impl From<http::Error> for Error {
    fn from(e: http::Error) -> Self {
        Error::Http(e)
    }
}

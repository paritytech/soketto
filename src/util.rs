//! General Utilities
use std::borrow::Cow;
use std::io;
#[cfg(test)]
use std::io::Write;

#[cfg(test)]
pub fn stdo(msg: &str) {
    writeln!(io::stdout(), "{}", msg).expect("Unable to write to stdout!");
}

/// Generate an `io::ErrorKind::Other` error with the given description.
pub fn other(desc: &str) -> io::Error {
    io::Error::new(io::ErrorKind::Other, desc)
}

/// Generate a formatted hex string from a vector of bytes.
pub fn as_hex(buf: &[u8]) -> String {
    let mut hexy = String::new();

    for (idx, byte) in buf.iter().enumerate() {
        if idx % 16 == 0 {
            hexy.push_str(&format!("{:08x}:", idx))
        }

        hexy.push_str(&format!(" 0x{:02x}", byte));

        if idx % 16 == 15 {
            hexy.push('\n');
        }
    }

    hexy
}

/// Generate the hex string header.
pub fn hex_header() -> String {
    format!("{:8} {:5}{:5}{:5}{:5}{:5}{:5}{:5}{:5}{:5}{:5}{:>5}{:>5}{:>5}{:>5}{:>5}{:>5}",
            "Address",
            0,
            1,
            2,
            3,
            4,
            5,
            6,
            7,
            8,
            9,
            "a",
            "b",
            "c",
            "d",
            "e",
            "f")
}

pub struct Invalid<'a>(pub(crate) Cow<'a, str>);

pub(crate) fn expect_header<'a, T>(r: &http::Request<T>, n: &http::header::HeaderName, v: &str) -> Result<(), Invalid<'a>> {
    with_header(r, n, move |value| {
        if unicase::Ascii::new(value) != v {
            Err(Invalid(Cow::Owned(format!("unexpected header value: {}", n))))
        } else {
            Ok(())
        }
    })
}

pub(crate) fn with_header<'a, T, F, R>(r: &http::Request<T>, n: &http::header::HeaderName, f: F) -> Result<R, Invalid<'a>>
where
    F: Fn(&str) -> Result<R, Invalid<'a>>
{
    r.headers().get(n)
        .ok_or(Invalid(Cow::Owned(format!("missing header name: {}", n))))
        .and_then(|value| {
            value.to_str().map_err(|_| Invalid(Cow::Owned(format!("invalid header: {}", n))))
        })
        .and_then(f)
}


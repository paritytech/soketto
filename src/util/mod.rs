//! General Utilities
use std::io;
#[cfg(test)]
use std::io::Write;

#[allow(dead_code)]
pub mod utf8;

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

//! General Utilities
use std::io;

/// Generate an `io::ErrorKind::Other` error with the given description.
pub fn other(desc: &str) -> io::Error {
    io::Error::new(io::ErrorKind::Other, desc)
}

/// Generate a formatted hex string from a vector of bytes.
pub fn as_hex(buf: &[u8]) -> String {
    let mut hexy = String::new();

    hexy.push_str(&header());
    hexy.push('\n');

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
fn header() -> String {
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

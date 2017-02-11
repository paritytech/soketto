use std::io;

pub fn other(desc: &str) -> io::Error {
    io::Error::new(io::ErrorKind::Other, desc)
}

#[cfg(test)]
pub fn as_hex(buf: &Vec<u8>) -> String {
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

#[cfg(test)]
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

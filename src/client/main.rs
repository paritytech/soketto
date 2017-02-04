use std::io::{Read, Write};
use std::net::TcpStream;

fn main() {
    let mut stream = TcpStream::connect("127.0.0.1:3000").unwrap();
    let mut buf = Vec::new();
    // ignore the Result
    let _ = stream.write_all(&[0x88, 0x81, 0x00, 0x00, 0x00, 0x00, 0x00]);
    let _ = stream.read_to_end(&mut buf);
    println!("buf: {:?}", buf);
}

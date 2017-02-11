extern crate native_tls;

use native_tls::TlsConnector;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::{thread, time};

fn as_hex(buf: &[u8]) -> String {
    let mut hexy = String::from("buf: [");
    for byte in buf {
        hexy.push_str(&format!(" 0x{:02x}", byte));
    }
    hexy.push_str(" ]");
    hexy
}

fn main() {
    let connector = TlsConnector::builder().unwrap().build().unwrap();
    let stream = TcpStream::connect("jasonozias.com:32276").unwrap();
    let mut stream = connector.connect("jasonozias.com", stream).unwrap();
    let mut buf = [0; 2];

    let ping = [0x89, 0x00];
    println!("Sending Ping (No Data): {}", as_hex(&ping));
    stream.write_all(&ping).unwrap();
    if let Ok(()) = stream.read_exact(&mut buf) {
        assert!(buf == [0x8a, 0x00]);
        println!("Got Pong: {}", as_hex(&buf));
    } else {
        println!("unable to read buf!");
    }

    let half_second = time::Duration::from_millis(500);
    thread::sleep(half_second);

    // Close from Client Data Frame
    let close = [0x88, 0x00];
    println!("Sending Close (No Data): {}", as_hex(&close));
    let _ = stream.write_all(&close);
    if let Ok(()) = stream.read_exact(&mut buf) {
        assert!(buf == [0x88, 0x00]);
        println!("{}", as_hex(&buf));
    } else {
        println!("unable to read buf!");
    }

    // We should be closed so this should fail.
    if let Err(e) = stream.read_exact(&mut buf) {
        println!("An error here is good, means close is working! {}", e);
    }
}

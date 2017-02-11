extern crate httparse;

use httparse::{EMPTY_HEADER, Response};
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
    let mut stream = TcpStream::connect("10.0.0.200:11579").unwrap();
    let mut buf = [0; 2];

    let request = "\
        GET / HTTP/1.1\r\n\
        Host: jasonozias.com\r\n\
        Upgrade: websocket\r\n\
        Connection: upgrade\r\n\
        Sec-WebSocket-Key: upgrade\r\n\
        Sec-WebSocket-Version: 13\r\n\
        \r\n";

    println!("Sending Client Handshake Request");
    let mut rb = Vec::new();
    let _ = stream.write_all(request.as_bytes());

    loop {
        let mut resp_buf = [0; 2048];
        let mut headers = [EMPTY_HEADER; 32];
        let mut resp = Response::new(&mut headers);

        if let Ok(count) = stream.read(&mut resp_buf) {
            let tmp = resp_buf[0..count].to_vec();
            rb.extend(tmp);
        }

        if let Ok(res) = resp.parse(&rb) {
            if res.is_partial() {
                continue;
            } else if res.is_complete() {
                println!("VALID RESPONSE: {:?}", resp.code);
                for header in resp.headers.iter() {
                    println!("{}: {}", header.name, String::from_utf8_lossy(header.value));
                }
                break;
            }
        }
    }

    let half_second = time::Duration::from_millis(500);
    thread::sleep(half_second);

    // Copy from Client Data Frame
    let ping = [0x89, 0x00];
    println!("Sending Ping (No Data): {}", as_hex(&ping));
    let _ = stream.write_all(&ping);
    if let Ok(()) = stream.read_exact(&mut buf) {
        println!("buf: {:?}", buf);
        assert!(buf == [0x8a, 0x00]);
        println!("Got Pong: {}", as_hex(&buf));
    } else {
        println!("unable to read buf!");
    }

    // let _ = stream.write_all(&[0x02, 0x81, 0x00, 0x00, 0x00, 0x01, 0x00]);
    // thread::sleep(half_second);
    // let _ = stream.write_all(&[0x00, 0x81, 0x00, 0x00, 0x00, 0x01, 0x01]);
    // thread::sleep(half_second);
    // let _ = stream.write_all(&[0x80, 0x81, 0x00, 0x00, 0x00, 0x01, 0x02]);
    //
    // if let Ok(()) = stream.read_exact(&mut buf) {
    //     print!("buf: [");
    //     for byte in &buf {
    //         print!(" {:02x}", byte);
    //     }
    //     println!(" ]");
    // } else {
    //     println!("unable to read buf!");
    // }

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
    // thread::sleep(half_second);
    // let _ = stream.write_all(&[0x02, 0x81, 0x00, 0x00, 0x00, 0x01, 0x00]);
    // thread::sleep(half_second);
    // let _ = stream.write_all(&[0x00, 0x81, 0x00, 0x00, 0x00, 0x01, 0x01]);
    // thread::sleep(time::Duration::from_millis(200));
    // // This should return immediately.
    // let _ = stream.write_all(&[0x88, 0x81, 0x00, 0x00, 0x00, 0x00, 0x00]);
    //
    // if let Ok(()) = stream.read_exact(&mut buf) {
    //     println!("buf: {:?}", buf);
    // } else {
    //     println!("unable to read buf!");
    // }
    //
    // thread::sleep(half_second);
    // let _ = stream.write_all(&[0x80, 0x81, 0x00, 0x00, 0x00, 0x01, 0x02]);
    //
    // if let Ok(()) = stream.read_exact(&mut buf) {
    //     println!("buf: {:?}", buf);
    // } else {
    //     println!("unable to read buf!");
    // }
}

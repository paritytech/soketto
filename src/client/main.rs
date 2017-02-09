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
    let mut stream = TcpStream::connect("127.0.0.1:3000").unwrap();
    let mut buf = [0; 2];

    // Copy from Client Data Frame
    let ping = [0x89, 0x00];
    println!("Sending Ping (No Data): {}", as_hex(&ping));
    let _ = stream.write_all(&ping);
    if let Ok(()) = stream.read_exact(&mut buf) {
        assert!(buf[0] == 0x8a);
        println!("Got Pong: {}", as_hex(&buf));
    } else {
        println!("unable to read buf!");
    }

    let half_second = time::Duration::from_millis(500);
    thread::sleep(half_second);

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
    thread::sleep(half_second);
    let close = [0x88, 0x00];
    println!("Sending Close (No Data): {}", as_hex(&close));
    let _ = stream.write_all(&close);
    if let Ok(()) = stream.read_exact(&mut buf) {
        println!("{}", as_hex(&buf));
    } else {
        println!("unable to read buf!");
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

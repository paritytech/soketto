use std::io::{Read, Write};
use std::net::TcpStream;
use std::{thread, time};

fn main() {
    let mut stream = TcpStream::connect("127.0.0.1:3000").unwrap();
    let mut buf = [0; 2];
    // ignore the Result
    let _ = stream.write_all(&[0x88, 0x81, 0x00, 0x00, 0x00, 0x00, 0x00]);
    if let Ok(()) = stream.read_exact(&mut buf) {
        println!("buf: {:?}", buf);
    } else {
        println!("unable to read buf!");
    }

    let half_second = time::Duration::from_millis(500);
    thread::sleep(half_second);
    let _ = stream.write_all(&[0x02, 0x81, 0x00, 0x00, 0x00, 0x01, 0x00]);
    thread::sleep(half_second);
    let _ = stream.write_all(&[0x00, 0x81, 0x00, 0x00, 0x00, 0x01, 0x01]);
    thread::sleep(half_second);
    let _ = stream.write_all(&[0x80, 0x81, 0x00, 0x00, 0x00, 0x01, 0x02]);

    if let Ok(()) = stream.read_exact(&mut buf) {
        println!("buf: {:?}", buf);
    } else {
        println!("unable to read buf!");
    }

    thread::sleep(half_second);
    let _ = stream.write_all(&[0x02, 0x81, 0x00, 0x00, 0x00, 0x01, 0x00]);
    thread::sleep(half_second);
    let _ = stream.write_all(&[0x00, 0x81, 0x00, 0x00, 0x00, 0x01, 0x01]);
    thread::sleep(time::Duration::from_millis(200));
    // This should return immediately.
    let _ = stream.write_all(&[0x88, 0x81, 0x00, 0x00, 0x00, 0x00, 0x00]);

    if let Ok(()) = stream.read_exact(&mut buf) {
        println!("buf: {:?}", buf);
    } else {
        println!("unable to read buf!");
    }

    thread::sleep(half_second);
    let _ = stream.write_all(&[0x80, 0x81, 0x00, 0x00, 0x00, 0x01, 0x02]);

    if let Ok(()) = stream.read_exact(&mut buf) {
        println!("buf: {:?}", buf);
    } else {
        println!("unable to read buf!");
    }
}

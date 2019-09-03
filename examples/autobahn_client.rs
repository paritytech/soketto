// Copyright (c) 2019 Parity Technologies (UK) Ltd.
//
// Licensed under the Apache License, Version 2.0
// <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0> or the MIT
// license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. All files in the project carrying such notice may not be copied,
// modified, or distributed except according to those terms.

// Example to be used with the autobahn test suite, a fully automated test
// suite to verify client and server implementations of websocket
// implementation.
//
// Once started, the tests can be executed with: wstest -m fuzzingserver
//
// See https://github.com/crossbario/autobahn-testsuite for details.

use assert_matches::assert_matches;
use async_std::{net::TcpStream, task};
use soketto::{BoxedError, connection, handshake};
use std::str::FromStr;

const SOKETTO_VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() -> Result<(), BoxedError> {
    env_logger::init();
    task::block_on(async {
        let n = num_of_cases().await?;
        for i in 1 ..= n {
            if let Err(e) = run_case(i).await {
                log::error!("case {}: {:?}", i, e)
            }
        }
        update_report().await?;
        Ok(())
    })
}

async fn num_of_cases() -> Result<usize, BoxedError> {
    let s = TcpStream::connect("127.0.0.1:9001").await?;
    let mut c = new_client(s, "/getCaseCount");
    assert_matches!(c.handshake().await?, handshake::ServerResponse::Accepted {..});
    let mut c = c.into_connection(true);
    let (payload, is_text) = c.receive().await?;
    assert!(is_text);
    let num = usize::from_str(std::str::from_utf8(&payload)?)?;
    log::info!("{} cases to run", num);
    Ok(num)
}

async fn run_case(n: usize) -> Result<(), BoxedError> {
    log::info!("running case {}", n);
    let resource = format!("/runCase?case={}&agent=soketto-{}", n, SOKETTO_VERSION);
    let s = TcpStream::connect("127.0.0.1:9001").await?;
    let mut c = new_client(s, &resource);
    assert_matches!(c.handshake().await?, handshake::ServerResponse::Accepted {..});
    let mut c = c.into_connection(true);
    c.validate_utf8(true);
    loop {
        match c.receive().await {
            Ok((mut payload, is_text)) =>
                if is_text {
                    c.send_text(&mut payload).await?
                } else {
                    c.send_binary(&mut payload).await?
                }
            Err(connection::Error::Closed) => return Ok(()),
            Err(e) => return Err(e.into())
        }
    }
}

async fn update_report() -> Result<(), BoxedError> {
    log::info!("requesting report generation");
    let resource = format!("/updateReports?agent=soketto-{}", SOKETTO_VERSION);
    let s = TcpStream::connect("127.0.0.1:9001").await?;
    let mut c = new_client(s, &resource);
    assert_matches!(c.handshake().await?, handshake::ServerResponse::Accepted {..});
    c.into_connection(true).close().await?;
    Ok(())
}

#[cfg(not(feature = "deflate"))]
fn new_client(socket: TcpStream, path: &str) -> handshake::Client<'_, TcpStream> {
    handshake::Client::new(socket, "127.0.0.1:9001", path)
}

#[cfg(feature = "deflate")]
fn new_client(socket: TcpStream, path: &str) -> handshake::Client<'_, TcpStream> {
    let mut client = handshake::Client::new(socket, "127.0.0.1:9001", path);
    let deflate = soketto::extension::deflate::Deflate::new(soketto::connection::Mode::Client);
    client.add_extension(Box::new(deflate));
    client
}


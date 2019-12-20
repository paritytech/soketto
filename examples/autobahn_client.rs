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
    let socket = TcpStream::connect("127.0.0.1:9001").await?;
    let mut client = new_client(socket, "/getCaseCount");
    assert_matches!(client.handshake().await?, handshake::ServerResponse::Accepted {..});
    let (_, mut receiver) = client.into_builder().finish();
    let data = receiver.receive_data().await?;
    assert!(data.is_text());
    let num = usize::from_str(std::str::from_utf8(data.as_ref())?)?;
    log::info!("{} cases to run", num);
    Ok(num)
}

async fn run_case(n: usize) -> Result<(), BoxedError> {
    log::info!("running case {}", n);
    let resource = format!("/runCase?case={}&agent=soketto-{}", n, SOKETTO_VERSION);
    let socket = TcpStream::connect("127.0.0.1:9001").await?;
    let mut client = new_client(socket, &resource);
    assert_matches!(client.handshake().await?, handshake::ServerResponse::Accepted {..});
    let (mut sender, mut receiver) = client.into_builder().finish();
    loop {
        match receiver.receive_data().await {
            Ok(mut data) => {
                if data.is_binary() {
                    sender.send_binary_mut(&mut data).await?
                } else {
                    sender.send_text(std::str::from_utf8(data.as_ref())?).await?
                }
                sender.flush().await?
            }
            Err(connection::Error::Closed) => return Ok(()),
            Err(e) => return Err(e.into())
        }
    }
}

async fn update_report() -> Result<(), BoxedError> {
    log::info!("requesting report generation");
    let resource = format!("/updateReports?agent=soketto-{}", SOKETTO_VERSION);
    let socket = TcpStream::connect("127.0.0.1:9001").await?;
    let mut client = new_client(socket, &resource);
    assert_matches!(client.handshake().await?, handshake::ServerResponse::Accepted {..});
    client.into_builder().finish().0.close().await?;
    Ok(())
}

#[cfg(not(feature = "deflate"))]
fn new_client(socket: TcpStream, path: &str) -> handshake::Client<'_, TcpStream> {
    handshake::Client::new(socket, "127.0.0.1:9001", path)
}

#[cfg(feature = "deflate")]
fn new_client(socket: TcpStream, path: &str) -> handshake::Client<'_, TcpStream> {
    let mut client = handshake::Client::new(socket, "127.0.0.1:9001", path);
    let deflate = soketto::extension::deflate::Deflate::new(soketto::Mode::Client);
    client.add_extension(Box::new(deflate));
    client
}


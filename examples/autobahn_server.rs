// Example to be used with the autobahn test suite, a fully automated test
// suite to verify client and server implementations of websocket
// implementation.
//
// Once started, the tests can be executed with: wstest -m fuzzingclient
//
// See https://github.com/crossbario/autobahn-testsuite for details.

use futures::{future::{self, Either}, prelude::*};
use std::{borrow::Cow, error, io};
use tokio::codec::{Framed, FramedParts};
use tokio::net::TcpListener;
use twist::{base, handshake, Connection, Mode};

fn main() {
    env_logger::init();
    let addr = "127.0.0.1:9001".parse().unwrap();
    let listener = TcpListener::bind(&addr).expect("TCP listener binds");

    let server = listener.incoming()
        .map_err(|e| eprintln!("accept failed = {:?}", e))
        .for_each(|socket| {
            let future = tokio::codec::Framed::new(socket, handshake::Server::new())
                .into_future()
                .map_err(|(e, _)| Box::new(e) as Box<dyn error::Error + Send>)
                .and_then(|(request, framed)| {
                    if let Some(r) = request {
                        let f = framed.send(Ok(handshake::Accept::new(Cow::Owned(r.key().into()))))
                            .map(|framed| {
                                let codec = base::Codec::new();
                                let old = framed.into_parts();
                                let mut new = FramedParts::new(old.io, codec);
                                new.read_buf = old.read_buf;
                                new.write_buf = old.write_buf;
                                let framed = Framed::from_parts(new);
                                Connection::from_framed(framed, Mode::Server)
                            });
                        Either::A(f.map_err(|e| Box::new(e) as Box<dyn error::Error + Send>))
                    } else {
                        let e: io::Error = io::ErrorKind::ConnectionAborted.into();
                        Either::B(future::err(Box::new(e) as Box<dyn error::Error + Send>))
                    }
                })
                .and_then(|connection| {
                    let (sink, stream) = connection.split();
                    let sink = sink.with(|data: base::Data| {
                        if data.is_text() {
                            std::str::from_utf8(data.as_ref())?;
                        }
                        Ok(data)
                    });
                    stream.forward(sink)
                        .and_then(|(_stream, mut sink)| future::poll_fn(move || sink.close()))
                        .map_err(|e| Box::new(e) as Box<dyn error::Error + Send>)
                });
            tokio::spawn(future.map_err(|e| eprintln!("{:?}", e)))
        });

    tokio::run(server)
}


// Copyright (c) 2016 twist developers
//
// Licensed under the Apache License, Version 2.0
// <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0> or the MIT
// license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. All files in the project carrying such notice may not be copied,
// modified, or distributed except according to those terms.

//! An implementation of the [RFC6455][rfc6455] websocket protocol as a set
//! of tokio codecs.
//!
//! [rfc6455]: https://tools.ietf.org/html/rfc6455

pub mod base;
pub mod handshake;
pub mod connection;

pub use connection::Connection;

#[cfg(test)]
mod tests {
    use futures::{future::{self, Either}, prelude::*};
    use super::*;
    use std::{borrow::Cow, error, io};
    use tokio::codec::{Framed, FramedParts};
    use tokio::net::TcpListener;

    #[test]
    fn autobahn_server() {
        let _ = env_logger::try_init();
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
                                    Connection::from(framed)
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
}

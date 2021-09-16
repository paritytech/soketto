// Copyright (c) 2021 Parity Technologies (UK) Ltd.
//
// Licensed under the Apache License, Version 2.0
// <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0> or the MIT
// license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. All files in the project carrying such notice may not be copied,
// modified, or distributed except according to those terms.

// An example of how to use of Soketto alongside Hyper, so that we can handle
// standard HTTP traffic with Hyper, and WebSocket connections with Soketto, on
// the same port.
//
// To try this, start up the example (`cargo run --example hyper_server`) and then
// navigate to localhost:3000 and, in the browser JS console, run:
//
// ```
// var socket = new WebSocket("ws://localhost:3000");
// socket.onmessage = function(msg) { console.log(msg) };
// socket.send("Hello!");
// ```
//
// You'll see any messages you send echoed back.

use futures::io::{BufReader, BufWriter};
use hyper::{Body, Request, Response};
use soketto::{
	handshake::{
		self,
		http::{negotiate_upgrade, NegotiationError},
	},
	BoxedError,
};
use tokio_util::compat::TokioAsyncReadCompatExt;

/// Start up a hyper server.
#[tokio::main]
async fn main() -> Result<(), BoxedError> {
	let addr = ([127, 0, 0, 1], 3000).into();

	let service =
		hyper::service::make_service_fn(|_| async { Ok::<_, hyper::Error>(hyper::service::service_fn(handler)) });
	let server = hyper::Server::bind(&addr).serve(service);

	println!("Listening on http://{} — connect and I'll echo back anything you send!", server.local_addr());
	server.await?;

	Ok(())
}

/// Our SokettoServer in this example is basically a hyper `Upgraded` stream with a BufReader and BufWriter wrapped
/// around it for performance.
type SokettoServer<'a> =
	handshake::Server<'a, BufReader<BufWriter<tokio_util::compat::Compat<hyper::upgrade::Upgraded>>>>;

/// Handle incoming HTTP Requests.
async fn handler(req: Request<Body>) -> Result<hyper::Response<Body>, BoxedError> {
	match negotiate_upgrade(&req) {
		// The upgrade request was successful, so reply with the appropriate response and start a Soketto server up:
		Ok(upgrade_negotiation) => {
// BUG: So, this all looks good in theory, and gives us a way to neatly configure a server based on the incoming request,
// and ensure that the response contains the correct information. BUT! `hyper::upgrade::on` waits until the response is sent
// (I think) until it hands back the stream, so it doesn't actually work! Will need a(nother) rethink..

			// The negotiation to upgrade to a WebSocket connection has been successful so far. Next, we get back the underlying
			// stream using `hyper::upgrade::on`, and hand this to a Soketto server to use to handle the WebSocket communication
			// on this socket.
			let stream = hyper::upgrade::on(req).await?;
			let mut server = handshake::Server::new(BufReader::new(BufWriter::new(stream.compat())));

			// Now that we have a server, we can add extensions to it if we like.
			#[cfg(feature = "deflate")]
			{
				let deflate = soketto::extension::deflate::Deflate::new(soketto::Mode::Server);
				server.add_extension(Box::new(deflate));
			}

			// Given a configured server, we can now conclude our upgrade negotiation by getting back a response to hand back to the client.
			// This may lead to some configuration of enabled extensions based on the incoming request params, and will fail if the configuration
			// parameters are invalid for the extionsions in question.
			let response = match upgrade_negotiation.into_response(&mut server) {
				Ok(res) => res,
				Err(_e) => {
					println!("aaah {:?}", _e);
					return Ok(Response::new(Body::from("Something went wrong upgrading!")))
				},
			};

			// Spawn off a task to handle the long running socket server that we've established.
			tokio::spawn(async move {
				if let Err(e) = websocket_echo_messages(server).await {
					eprintln!("Error upgrading to websocket connection: {}", e);
				}
			});

			// Return the response ("fixing" the body type, since `negotiate_upgrade` doesn't know about `hyper::Body`).
			Ok(response.map(|()| Body::empty()))
		}
		// We tried to upgrade and failed early on; tell the client about the failure however we like:
		Err(NegotiationError::HandshakeError(_e)) => Ok(Response::new(Body::from("Something went wrong upgrading!"))),
		// The request wasn't an upgrade request; let's treat it as a standard HTTP request:
		Err(NegotiationError::NotAnUpgradeRequest) => Ok(Response::new(Body::from("Hello HTTP!"))),
	}
}

/// Echo any messages we get from the client back to them
async fn websocket_echo_messages(server: SokettoServer<'_>) -> Result<(), BoxedError> {
	// Get back a reader and writer that we can use to send and receive websocket messages.
	let (mut sender, mut receiver) = server.into_builder().finish();

	// Echo any received messages back to the client:
	let mut message = Vec::new();
	loop {
		message.clear();
		match receiver.receive_data(&mut message).await {
			Ok(soketto::Data::Binary(n)) => {
				assert_eq!(n, message.len());
				sender.send_binary_mut(&mut message).await?;
				sender.flush().await?
			}
			Ok(soketto::Data::Text(n)) => {
				assert_eq!(n, message.len());
				if let Ok(txt) = std::str::from_utf8(&message) {
					sender.send_text(txt).await?;
					sender.flush().await?
				} else {
					break;
				}
			}
			Err(soketto::connection::Error::Closed) => break,
			Err(e) => {
				eprintln!("Websocket connection error: {}", e);
				break;
			}
		}
	}

	Ok(())
}

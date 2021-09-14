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
		http::{upgrade_request, UpgradeError, Upgraded},
		server,
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

	println!("Listening on http://{}", server.local_addr());
	server.await?;

	Ok(())
}

/// Handle incoming HTTP Requests.
async fn handler(req: Request<Body>) -> Result<hyper::Response<Body>, BoxedError> {
	match upgrade_request(&req) {
		// The upgrade was successful, so we return the response and start up a server to take charge of the socket:
		Ok(Upgraded { response, server_configuration }) => {
			tokio::spawn(async move {
				let on_upgrade = hyper::upgrade::on(req);
				if let Err(e) = websocket_echo_messages(on_upgrade, server_configuration).await {
					eprintln!("Error upgrading to websocket connection: {}", e);
				}
			});
			Ok(response.map(|()| Body::empty()))
		}
		// We tried to upgrade and failed; tell the client about the failure however we like:
		Err(UpgradeError::HandshakeError(_e)) => Ok(Response::new(Body::from("Something went wrong upgrading!"))),
		// The request wasn't an upgrade request; let's treat it as a standard HTTP request:
		Err(UpgradeError::NotAnUpgradeRequest) => Ok(Response::new(Body::from("Hello HTTP!"))),
	}
}

/// Echo any messages we get from the client back to them
async fn websocket_echo_messages(
	on_upgrade: hyper::upgrade::OnUpgrade,
	server_configuration: server::ServerConfiguration,
) -> Result<(), BoxedError> {
	// Wait for the request to upgrade, and pass the stream we get back to Soketto to handle the WS connection:
	let stream = on_upgrade.await?;
	let mut server = handshake::Server::new(BufReader::new(BufWriter::new(stream.compat())));
	server.configure(server_configuration)?;
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

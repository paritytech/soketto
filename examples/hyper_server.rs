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
// You'll see any messages you send achoed back.

use futures::io::{BufReader, BufWriter};
use hyper::{Body, Request, Response};
use soketto::{handshake, BoxedError};
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
	// If the request is asking to be upgraded to a websocket connection, we do that,
	// and we handle the websocket connection (in this case, by echoing messages back):
	if is_upgrade_request(&req) {
		let (res, on_upgrade) = upgrade_to_websocket(req)?;
		tokio::spawn(async move {
			if let Err(e) = websocket_echo_messages(on_upgrade).await {
				eprintln!("Error upgrading to websocket connection: {}", e);
			}
		});
		Ok(res)
	}
	// Or, we can handle the request as a standard HTTP request:
	else {
		Ok(Response::new(Body::from("Hello HTTP!")))
	}
}

/// Return the response to the upgrade request, and a way to get hold of the underlying TCP stream
fn upgrade_to_websocket(req: Request<Body>) -> Result<(Response<Body>, hyper::upgrade::OnUpgrade), handshake::Error> {
	let key = req.headers().get("Sec-WebSocket-Key").ok_or(handshake::Error::InvalidSecWebSocketAccept)?;
	if req.headers().get("Sec-WebSocket-Version").map(|v| v.as_bytes()) != Some(b"13") {
		return Err(handshake::Error::HeaderNotFound("Sec-WebSocket-Version".into()));
	}

	// Just a little ceremony we need to go to to return the correct response key:
	let mut accept_key_buf = [0; 32];
	let accept_key = generate_websocket_accept_key(key.as_bytes(), &mut accept_key_buf);

	let response = Response::builder()
		.status(hyper::StatusCode::SWITCHING_PROTOCOLS)
		.header(hyper::header::CONNECTION, "upgrade")
		.header(hyper::header::UPGRADE, "websocket")
		.header("Sec-WebSocket-Accept", accept_key)
		.body(Body::empty())
		.expect("bug: failed to build response");

	Ok((response, hyper::upgrade::on(req)))
}

/// Echo any messages we get from the client back to them
async fn websocket_echo_messages(on_upgrade: hyper::upgrade::OnUpgrade) -> Result<(), BoxedError> {
	// Wait for the request to upgrade, and pass the stream we get back to Soketto to handle the WS connection:
	let stream = on_upgrade.await?;
	let server = handshake::Server::new(BufReader::new(BufWriter::new(stream.compat())));
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

/// Defined in RFC 6455. this is how we convert the Sec-WebSocket-Key in a request into a
/// Sec-WebSocket-Accept that we return in the response.
fn generate_websocket_accept_key<'a>(key: &[u8], buf: &'a mut [u8; 32]) -> &'a [u8] {
	// Defined in RFC 6455, we append this to the key to generate the response:
	const KEY: &[u8] = b"258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

	use sha1::{Digest, Sha1};
	let mut digest = Sha1::new();
	digest.update(key);
	digest.update(KEY);
	let d = digest.finalize();

	let n = base64::encode_config_slice(&d, base64::STANDARD, buf);
	&buf[..n]
}

/// Check if a request is a websocket upgrade request.
pub fn is_upgrade_request<B>(request: &hyper::Request<B>) -> bool {
	header_contains_value(request.headers(), hyper::header::CONNECTION, b"upgrade")
		&& header_contains_value(request.headers(), hyper::header::UPGRADE, b"websocket")
}

/// Check if there is a header of the given name containing the wanted value.
fn header_contains_value(headers: &hyper::HeaderMap, header: hyper::header::HeaderName, value: &[u8]) -> bool {
	pub fn trim(x: &[u8]) -> &[u8] {
		let from = match x.iter().position(|x| !x.is_ascii_whitespace()) {
			Some(i) => i,
			None => return &x[0..0],
		};
		let to = x.iter().rposition(|x| !x.is_ascii_whitespace()).unwrap();
		&x[from..=to]
	}

	for header in headers.get_all(header) {
		if header.as_bytes().split(|&c| c == b',').any(|x| trim(x).eq_ignore_ascii_case(value)) {
			return true;
		}
	}
	false
}

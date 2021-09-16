// Copyright (c) 2021 Parity Technologies (UK) Ltd.
//
// Licensed under the Apache License, Version 2.0
// <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0> or the MIT
// license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. All files in the project carrying such notice may not be copied,
// modified, or distributed except according to those terms.

/*!
This module exposes the utility method [`upgrade_request`] to make it easier to upgrade
an [`http::Request`] into a Soketto Socket connection. Take a look at the `examples/hyper_server.rs`
example in the crate repository to see this in action.
*/

use super::{Server, SEC_WEBSOCKET_EXTENSIONS};
use crate::handshake;
use http::{header, HeaderMap, Response};
use std::convert::TryInto;

/// An error attempting to upgrade the [`http::Request`]
pub enum NegotiationError {
	/// The [`http::Request`] provided wasn't a socket upgrade request.
	NotAnUpgradeRequest,
	/// A [`handshake::Error`] encountered attempting to upgrade the request.
	HandshakeError(handshake::Error),
}

impl From<handshake::Error> for NegotiationError {
	fn from(e: handshake::Error) -> Self {
		NegotiationError::HandshakeError(e)
	}
}

/// This is handed back on a successful call to [`negotiate_upgrade`]. It has one method,
/// [`Negotiation::into_response`], which can be provided a Soketto server, and hands back
/// a response to send to the client, as well as configuring the server extensions as needed
/// based on the request.
pub struct Negotiation {
	key: [u8; 24],
	extension_config: Vec<String>,
}

impl Negotiation {
	/// Generate an [`http::Response`] to the negotiation request. This should be
	/// returned to the client to complete the upgrade negotiation.
	pub fn into_response<'a, T>(self, server: &mut Server<'a, T>) -> Result<Response<()>, handshake::Error> {
		// Attempt to set the extension configuration params that the client requested.
		for config_str in self.extension_config {
			handshake::configure_extensions(server.extensions_mut(), &config_str)?;
		}

		let mut accept_key_buf = [0; 32];
		let accept_key = handshake::generate_accept_key(&self.key, &mut accept_key_buf);

		// Build a response that should be sent back to the client to acknowledge the upgrade.
		let mut response = Response::builder()
			.status(http::StatusCode::SWITCHING_PROTOCOLS)
			.header(http::header::CONNECTION, "upgrade")
			.header(http::header::UPGRADE, "websocket")
			.header("Sec-WebSocket-Accept", accept_key);

		// Tell the client about the agreed-upon extension configuration. We reuse code to build up the
		// extension header value, but that does make this a little more clunky.
		if !server.extensions_mut().is_empty() {
			let mut buf = bytes::BytesMut::new();
			let enabled_extensions = server.extensions_mut().iter().filter(|e| e.is_enabled()).peekable();
			handshake::append_extension_header_value(enabled_extensions, &mut buf);
			response = response.header("Sec-WebSocket-Extensions", buf.as_ref());
		}

		let response = response.body(()).expect("bug: failed to build response");
		Ok(response)
	}
}

/// Upgrade the provided [`http::Request`] to a socket connection. This returns an [`http::Response`]
/// that should be sent back to the client, as well as a [`ExtensionConfiguration`] struct which can be
/// handed to a Soketto server to configure its extensions/protocols based on this request.
pub fn negotiate_upgrade<B>(req: &http::Request<B>) -> Result<Negotiation, NegotiationError> {
	if !is_upgrade_request(&req) {
		return Err(NegotiationError::NotAnUpgradeRequest);
	}

	let key = match req.headers().get("Sec-WebSocket-Key") {
		Some(key) => key,
		None => {
			return Err(handshake::Error::HeaderNotFound("Sec-WebSocket-Key".into()).into());
		}
	};

	if req.headers().get("Sec-WebSocket-Version").map(|v| v.as_bytes()) != Some(b"13") {
		return Err(handshake::Error::HeaderNotFound("Sec-WebSocket-Version".into()).into());
	}

	// Pull out the Sec-WebSocket-Key; we'll need this for our response.
	let key: [u8; 24] = match key.as_bytes().try_into() {
		Ok(key) => key,
		Err(_) => return Err(NegotiationError::HandshakeError(handshake::Error::InvalidSecWebSocketAccept)),
	};

	// Get extension information out of the request as we'll need this as well.
	let extension_config = req
		.headers()
		.iter()
		.filter(|&(name, _)| name.as_str().eq_ignore_ascii_case(SEC_WEBSOCKET_EXTENSIONS))
		.map(|(_, value)| Ok(std::str::from_utf8(value.as_bytes())?.to_string()))
		.collect::<Result<Vec<_>, handshake::Error>>()?;

	Ok(Negotiation { key, extension_config })
}

/// Check if a request looks like a websocket upgrade request.
fn is_upgrade_request<B>(request: &http::Request<B>) -> bool {
	header_contains_value(request.headers(), header::CONNECTION, b"upgrade")
		&& header_contains_value(request.headers(), header::UPGRADE, b"websocket")
}

/// Check if there is a header of the given name containing the wanted value.
fn header_contains_value(headers: &HeaderMap, header: header::HeaderName, value: &[u8]) -> bool {
	pub fn trim(x: &[u8]) -> &[u8] {
		let from = match x.iter().position(|x| !x.is_ascii_whitespace()) {
			Some(i) => i,
			None => return &[],
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

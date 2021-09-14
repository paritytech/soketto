// Copyright (c) 2019 Parity Technologies (UK) Ltd.
//
// Licensed under the Apache License, Version 2.0
// <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0> or the MIT
// license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. All files in the project carrying such notice may not be copied,
// modified, or distributed except according to those terms.

//! This module exposes a utility method and a couple of related types to
//! make it easier to upgrade an [`http::Request`] into a Soketto Socket
//! connection. Take a look at the `examples/hyper_server` example to see it
//! in use.

use super::SEC_WEBSOCKET_EXTENSIONS;
use crate::handshake;
use crate::handshake::server::ServerConfiguration;
use http::{header, HeaderMap, Response};
use std::convert::TryInto;

/// An error attempting to upgrade the [`http::Request`]
pub enum UpgradeError {
	/// The [`http::Request`] provided wasn't a socket upgrade request.
	NotAnUpgradeRequest,
	/// A [`handshake::Error`] encountered attempting to upgrade the request.
	HandshakeError(handshake::Error),
}

impl From<handshake::Error> for UpgradeError {
	fn from(e: handshake::Error) -> Self {
		UpgradeError::HandshakeError(e)
	}
}

/// This is handed back on a successful call to [`upgrade_request`].
pub struct Upgraded {
	/// This should be handed back to the client to complete the upgrade.
	pub response: http::Response<()>,
	/// This should be passed to a [`handshake::Server`] once it's been
	/// constructed, to configure it according to this request.
	pub server_configuration: ServerConfiguration,
}

/// Upgrade the provided [`http::Request`] to a socket connection. This returns an [`http::Response`]
/// that should be sent back to the client, as well as a [`ServerConfiguration`] struct which can be
/// handed to a Soketto server to configure its extensions/protocols based on this request.
pub fn upgrade_request<B>(req: &http::Request<B>) -> Result<Upgraded, UpgradeError> {
	if !is_upgrade_request(&req) {
		return Err(UpgradeError::NotAnUpgradeRequest);
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

	// Given the bytes provided in Sec-WebSocket-Key, generate the bytes to return in Sec-WebSocket-Accept:
	let mut accept_key_buf = [0; 32];
	let key = match key.as_bytes().try_into() {
		Ok(key) => key,
		Err(_) => return Err(UpgradeError::HandshakeError(handshake::Error::InvalidSecWebSocketAccept)),
	};
	let accept_key = handshake::generate_accept_key(key, &mut accept_key_buf);

	// Get extension information out of the request.
	let extension_config = req
		.headers()
		.iter()
		.filter(|&(name, _)| name.as_str().eq_ignore_ascii_case(SEC_WEBSOCKET_EXTENSIONS))
		.map(|(_, value)| Ok(std::str::from_utf8(value.as_bytes())?.to_string()))
		.collect::<Result<Vec<_>, handshake::Error>>()?;
	let server_configuration = ServerConfiguration { extension_config };

	// Build a response that should be sent back to the client to acknowledge the upgrade.
	let response = Response::builder()
		.status(http::StatusCode::SWITCHING_PROTOCOLS)
		.header(http::header::CONNECTION, "upgrade")
		.header(http::header::UPGRADE, "websocket")
		.header("Sec-WebSocket-Accept", accept_key)
		.body(())
		.expect("bug: failed to build response");

	Ok(Upgraded { response, server_configuration })
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

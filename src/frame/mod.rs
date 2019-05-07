//! A websocket frame.

pub mod base;
pub mod client;
pub mod server;

use base::OpCode;

/// A websocket frame.
#[derive(Debug, Clone)]
pub enum WebSocket {
    /// TODO
    Base(base::Frame),
    /// TODO
    ClientHandshake(client::request::Frame),
    /// TODO
    ClientHandshakeResponse(client::response::Frame),
    /// TODO
    ServerHandshake(server::request::Frame),
    /// TODO
    ServerHandshakeResponse(server::response::Frame)
}

impl WebSocket {
    /// Create a pong frame.
    pub fn pong(app_data: Vec<u8>) -> WebSocket {
        let mut frame = base::Frame::default();
        frame.set_fin(true)
            .set_opcode(OpCode::Pong)
            .set_payload_length(app_data.len() as u64)
            .set_application_data(app_data);
        WebSocket::Base(frame)
    }

    /// Create a close frame.
    pub fn close(app_data: Vec<u8>) -> WebSocket {
        let mut frame = base::Frame::default();
        frame.set_fin(true).set_opcode(OpCode::Close)
            .set_payload_length(app_data.len() as u64)
            .set_application_data(app_data);
        WebSocket::Base(frame)
    }
}


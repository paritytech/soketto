pub mod base;
pub mod handshake;

use base::OpCode;

/// A websocket frame.
#[derive(Debug)]
pub enum WebSocket {
    /// The initial client to server handshake HTTP request.
    ClientRequest(handshake::client::Request),
    /// The server's handshake HTTP response.
    ServerResponse(handshake::server::Response),
    /// The server's handshake HTTP reeponse as received by the client.
    ClientResponse(handshake::client::Response),
    /// A generic post-handshake websocket frame
    Base(base::Frame)
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


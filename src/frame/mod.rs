pub mod base;
pub mod handshake;

use base::OpCode;

/// A websocket frame.
#[derive(Debug)]
pub enum WebSocket {
    /// The initial client to server handshake request.
    ClientRequest(handshake::client::Request),
    /// The server's handshake response.
    ServerResponse(handshake::server::Response),
    /// A generic post-handshake websocket frame
    Base(base::Frame)
}

impl WebSocket {
    /// Create a pong frame.
    pub fn pong(app_data: Vec<u8>) -> WebSocket {
        let mut header = base::Header::new(OpCode::Pong);
        header.set_fin(true);
        let mut frame = base::Frame::from(header);
        frame.set_application_data(app_data);
        WebSocket::Base(frame)
    }

    /// Create a close frame.
    pub fn close(app_data: Vec<u8>) -> WebSocket {
        let mut header = base::Header::new(OpCode::Close);
        header.set_fin(true);
        let mut frame = base::Frame::from(header);
        frame.set_application_data(app_data);
        WebSocket::Base(frame)
    }
}


//! A websocket frame.

use crate::frame::base::{Frame, OpCode};
use crate::frame::client::request::Frame as ClientsideHandshakeRequest;
use crate::frame::client::response::Frame as ClientsideHandshakeResponse;
use crate::frame::server::request::Frame as ServerSideHandshakeRequest;
use crate::frame::server::response::Frame as ServerSideHandshakeResponse;
use std::fmt;

pub mod base;
pub mod client;
pub mod server;

/// A websocket frame.
///
/// Note a websocket frame is either a client handshake frame, a
/// server handshake frame, or a base frame. They are mutually exclusive.
#[derive(Debug, Default, Clone)]
pub struct WebSocket {
    /// The client hanshake portion of a websocket frame.
    clientside_handshake_request: Option<ClientsideHandshakeRequest>,
    /// The server handshake response portion of a websoket frame.
    clientside_handshake_response: Option<ClientsideHandshakeResponse>,
    ///
    serverside_handshake_request: Option<ServerSideHandshakeRequest>,
    ///
    serverside_handshake_response: Option<ServerSideHandshakeResponse>,
    /// The base portion of a websocket frame.
    base: Option<Frame>,
}

impl WebSocket {
    /// Create a pong response frame.
    pub fn pong(app_data: Vec<u8>) -> WebSocket {
        let mut base: Frame = Default::default();
        base.set_fin(true).set_opcode(OpCode::Pong);
        base.set_payload_length(app_data.len() as u64)
            .set_application_data(app_data);

        WebSocket {
            base: Some(base),
            clientside_handshake_request: None,
            clientside_handshake_response: None,
            serverside_handshake_request: None,
            serverside_handshake_response: None,
        }
    }

    /// Create a close response frame.
    pub fn close(app_data: Vec<u8>) -> WebSocket {
        let mut base: Frame = Default::default();
        base.set_fin(true).set_opcode(OpCode::Close);
        base.set_payload_length(app_data.len() as u64);
        base.set_application_data(app_data);

        WebSocket {
            base: Some(base),
            clientside_handshake_request: None,
            clientside_handshake_response: None,
            serverside_handshake_request: None,
            serverside_handshake_response: None,
        }
    }

    /// Is this frame a close request frame?
    pub fn is_close(&self) -> bool {
        if let Some(ref base) = self.base {
            return base.opcode() == OpCode::Close;
        }
        false
    }

    /// Is this frame a close request frame?
    pub fn is_ping(&self) -> bool {
        if let Some(ref base) = self.base {
            return base.opcode() == OpCode::Ping;
        }
        false
    }

    /// Is this frame a close request frame?
    pub fn is_pong(&self) -> bool {
        if let Some(ref base) = self.base {
            return base.opcode() == OpCode::Pong;
        }
        false
    }

    /// Is this frame a serverside handshake request frame?
    pub fn is_serverside_handshake_request(&self) -> bool {
        self.serverside_handshake_request.is_some()
    }

    /// Is this frame a serverside handshake response frame?
    pub fn is_serverside_handshake_response(&self) -> bool {
        self.serverside_handshake_response.is_some()
    }

    /// Is this frame a clientside handshake reqeust frame?
    pub fn is_clientside_handshake_request(&self) -> bool {
        self.clientside_handshake_request.is_some()
    }

    /// Is this frame a server handshake response frame?
    pub fn is_clientside_handshake_response(&self) -> bool {
        self.clientside_handshake_response.is_some()
    }

    /// Is this frame the start of a fragmented set of frames?
    pub fn is_fragment_start(&self) -> bool {
        if let Some(ref base) = self.base {
            return !(base.opcode() == OpCode::Continue) && !base.fin();
        }
        false
    }

    /// Is the frame part of a fragmented set of frames?
    pub fn is_fragment(&self) -> bool {
        if let Some(ref base) = self.base {
            return base.opcode() == OpCode::Continue && !base.fin();
        }
        false
    }

    /// Does the continuation frame have a bad opcode?
    pub fn is_badfragment(&self) -> bool {
        if let Some(ref base) = self.base {
            return base.opcode() != OpCode::Continue && !base.opcode().is_control();
        }
        false
    }

    /// Is this frame the end of a fragmented set of frames?
    pub fn is_fragment_complete(&self) -> bool {
        if let Some(ref base) = self.base {
            return base.opcode() == OpCode::Continue && base.fin();
        }
        false
    }

    /// Pull the `Frame` out.
    pub fn clientside_handshake_request(&self) -> Option<&ClientsideHandshakeRequest> {
        if let Some(ref handshake) = self.clientside_handshake_request {
            Some(handshake)
        } else {
            None
        }
    }

    /// Set the `Frame`. Note that this will set the handshake to `None`, as they are
    /// mutually exclusive.
    pub fn set_clientside_handshake_request(&mut self,
                                            handshake: ClientsideHandshakeRequest)
                                            -> &mut WebSocket {
        self.clientside_handshake_request = Some(handshake);
        // Ensure mutually exculsive.
        self.base = None;
        self.clientside_handshake_response = None;
        self.serverside_handshake_request = None;
        self.serverside_handshake_response = None;
        self
    }

    /// Pull the `Frame` out.
    pub fn clientside_handshake_response(&self) -> Option<&ClientsideHandshakeResponse> {
        if let Some(ref handshake) = self.clientside_handshake_response {
            Some(handshake)
        } else {
            None
        }
    }

    /// Set the `Frame`. Note that this will set the handshake to `None`, as they are
    /// mutually exclusive.
    pub fn set_clientside_handshake_response(&mut self,
                                             handshake: ClientsideHandshakeResponse)
                                             -> &mut WebSocket {
        self.clientside_handshake_response = Some(handshake);
        // Ensure mutually exculsive.
        self.base = None;
        self.clientside_handshake_request = None;
        self.serverside_handshake_request = None;
        self.serverside_handshake_response = None;
        self
    }

    /// Pull the `Frame` out.
    pub fn serverside_handshake_request(&self) -> Option<&ServerSideHandshakeRequest> {
        if let Some(ref handshake) = self.serverside_handshake_request {
            Some(handshake)
        } else {
            None
        }
    }

    /// Set the `Frame`. Note that this will set the handshake to `None`, as they are
    /// mutually exclusive.
    pub fn set_serverside_handshake_request(&mut self,
                                            handshake: ServerSideHandshakeRequest)
                                            -> &mut WebSocket {
        self.serverside_handshake_request = Some(handshake);
        // Ensure mutually exculsive.
        self.base = None;
        self.clientside_handshake_request = None;
        self.clientside_handshake_response = None;
        self.serverside_handshake_response = None;
        self
    }

    /// Pull the `Frame` out.
    pub fn serverside_handshake_response(&self) -> Option<&ServerSideHandshakeResponse> {
        if let Some(ref handshake) = self.serverside_handshake_response {
            Some(handshake)
        } else {
            None
        }
    }

    /// Set the `Frame`. Note that this will set the handshake to `None`, as they are
    /// mutually exclusive.
    pub fn set_serverside_handshake_response(&mut self,
                                             handshake: ServerSideHandshakeResponse)
                                             -> &mut WebSocket {
        self.serverside_handshake_response = Some(handshake);
        // Ensure mutually exculsive.
        self.base = None;
        self.clientside_handshake_request = None;
        self.clientside_handshake_response = None;
        self.serverside_handshake_request = None;
        self
    }

    /// Get the `Frame`.
    pub fn base(&self) -> Option<&Frame> {
        if let Some(ref base) = self.base {
            Some(base)
        } else {
            None
        }
    }

    /// Set the `Frame`.  Note that this will set the handshake to `None`, as they are mutually
    /// exclusive.
    pub fn set_base(&mut self, base: Frame) -> &mut WebSocket {
        self.base = Some(base);
        // Ensure mutually exclusive.
        self.clientside_handshake_request = None;
        self.clientside_handshake_response = None;
        self.serverside_handshake_request = None;
        self.serverside_handshake_response = None;
        self
    }
}

impl fmt::Display for WebSocket {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if let Some(ref handshake) = self.clientside_handshake_request {
            write!(f, "{}", handshake)
        } else if let Some(ref handshake) = self.clientside_handshake_response {
            write!(f, "{}", handshake)
        } else if let Some(ref handshake) = self.serverside_handshake_request {
            write!(f, "{}", handshake)
        } else if let Some(ref handshake) = self.serverside_handshake_response {
            write!(f, "{}", handshake)
        } else if let Some(ref base) = self.base {
            write!(f, "{}", base)
        } else {
            write!(f, "Empty WebSocket Frame")
        }
    }
}

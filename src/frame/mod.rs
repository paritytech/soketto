//! A websocket server frame.
use frame::base::{Frame, OpCode};
use frame::client::handshake::Frame as ClientHandshakeFrame;
use frame::server::handshake::Frame as ServerHandshakeFrame;
use std::fmt;

pub mod base;
pub mod client;
pub mod server;

/// A `twist` websocket frame.  Note a websocket frame is either a client handshake frame, a
/// server handshake frame,  or a base frame.  They are mutually exclusive.
#[derive(Debug, Default, Clone)]
pub struct WebSocket {
    /// The client hanshake portion of a websocket frame.
    client_handshake: Option<ClientHandshakeFrame>,
    /// The server hanshake portion of a websocket frame.
    server_handshake: Option<ServerHandshakeFrame>,
    /// The base portion of a websocket frame.
    base: Option<Frame>,
}

impl WebSocket {
    /// Create a server handshake response frame.
    pub fn server_handshake_resp(handshake_frame: ServerHandshakeFrame) -> WebSocket {
        WebSocket {
            base: None,
            client_handshake: None,
            server_handshake: Some(handshake_frame),
        }
    }

    /// Create a pong response frame.
    pub fn pong(app_data: Option<Vec<u8>>) -> WebSocket {
        let mut base: Frame = Default::default();
        base.set_fin(true).set_opcode(OpCode::Pong);
        if let Some(app_data) = app_data {
            base.set_payload_length(app_data.len() as u64).set_application_data(Some(app_data));
        }

        WebSocket {
            base: Some(base),
            client_handshake: None,
            server_handshake: None,
        }
    }

    /// Create a close response frame.
    pub fn close(app_data: Option<Vec<u8>>) -> WebSocket {
        let mut base: Frame = Default::default();
        base.set_fin(true).set_opcode(OpCode::Close);
        if let Some(app_data) = app_data {
            base.set_payload_length(app_data.len() as u64);
            base.set_application_data(Some(app_data));
        }

        WebSocket {
            base: Some(base),
            client_handshake: None,
            server_handshake: None,
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

    /// Is this frame a server handshake frame?
    pub fn is_server_handshake(&self) -> bool {
        self.server_handshake.is_some()
    }

    /// Is this frame a client handshake frame?
    pub fn is_client_handshake(&self) -> bool {
        self.client_handshake.is_some()
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
    pub fn client_handshake(&self) -> Option<&ClientHandshakeFrame> {
        if let Some(ref handshake) = self.client_handshake {
            Some(handshake)
        } else {
            None
        }
    }

    /// Set the `Frame`. Note that this will set the handshake to `None`, as they are
    /// mutually exclusive.
    pub fn set_client_handshake(&mut self, handshake: ClientHandshakeFrame) -> &mut WebSocket {
        self.client_handshake = Some(handshake);
        // Ensure mutually exculsive.
        self.base = None;
        self.server_handshake = None;
        self
    }

    /// Pull the `Frame` out.
    pub fn server_handshake(&self) -> Option<&ServerHandshakeFrame> {
        if let Some(ref handshake) = self.server_handshake {
            Some(handshake)
        } else {
            None
        }
    }

    /// Set the `Frame`. Note that this will set the handshake to `None`, as they are
    /// mutually exclusive.
    pub fn set_server_handshake(&mut self, handshake: ServerHandshakeFrame) -> &mut WebSocket {
        self.server_handshake = Some(handshake);
        // Ensure mutually exculsive.
        self.base = None;
        self.client_handshake = None;
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
        self.server_handshake = None;
        self.client_handshake = None;
        self
    }
}

impl fmt::Display for WebSocket {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let hstr = if let Some(ref handshake) = self.server_handshake {
            format!("{}", handshake)
        } else {
            "None".to_string()
        };

        let bstr = if let Some(ref base) = self.base {
            format!("{}", base)
        } else {
            "None".to_string()
        };

        write!(f,
               "WebSocket {{\n\thandshake: {}\n\tbase: {}\n}}",
               hstr,
               bstr)
    }
}

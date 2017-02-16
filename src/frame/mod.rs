//! A websocket frame.
use frame::base::OpCode;
use std::fmt;

pub mod base;
pub mod handshake;

/// A struct representing a websocket frame.  Note a websocket frame is either a handshake frame or
/// a base frame.  They are mutually exclusive.
#[derive(Debug, Clone)]
pub struct WebSocket {
    /// The hanshake portion of a websocket frame.
    handshake: Option<handshake::Frame>,
    /// The base portion of a websocket frame.
    base: Option<base::Frame>,
}

impl WebSocket {
    /// Create a server handshake response frame.
    pub fn handshake_resp(handshake_frame: handshake::Frame) -> WebSocket {
        WebSocket {
            base: None,
            handshake: Some(handshake_frame),
        }
    }

    /// Create a pong response frame.
    pub fn pong(app_data: Option<Vec<u8>>) -> WebSocket {
        let mut base: base::Frame = Default::default();
        base.set_fin(true).set_opcode(OpCode::Pong);
        if let Some(app_data) = app_data {
            base.set_payload_length(app_data.len() as u64).set_application_data(Some(app_data));
        }

        WebSocket {
            base: Some(base),
            handshake: None,
        }
    }

    /// Create a close response frame.
    pub fn close(app_data: Option<Vec<u8>>) -> WebSocket {
        let mut base: base::Frame = Default::default();
        base.set_fin(true).set_opcode(OpCode::Close);
        if let Some(app_data) = app_data {
            base.set_payload_length(app_data.len() as u64);
            base.set_application_data(Some(app_data));
        }

        WebSocket {
            base: Some(base),
            handshake: None,
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

    /// Is this frame a handshake frame?
    pub fn is_handshake(&self) -> bool {
        self.handshake.is_some()
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
    pub fn handshake(&self) -> Option<&handshake::Frame> {
        if let Some(ref handshake) = self.handshake {
            Some(handshake)
        } else {
            None
        }
    }

    /// Set the `Frame`. Note that this will set the handshake to `None`, as they are
    /// mutually exclusive.
    pub fn set_handshake(&mut self, handshake: handshake::Frame) -> &mut WebSocket {
        self.handshake = Some(handshake);
        // Ensure mutually exculsive.
        self.base = None;
        self
    }

    /// Get the `Frame`.
    pub fn base(&self) -> Option<&base::Frame> {
        if let Some(ref base) = self.base {
            Some(base)
        } else {
            None
        }
    }

    /// Set the `Frame`.  Note that this will set the handshake to `None`, as they are mutually
    /// exclusive.
    pub fn set_base(&mut self, base: base::Frame) -> &mut WebSocket {
        self.base = Some(base);
        // Ensure mutually exclusive.
        self.handshake = None;
        self
    }
}

impl Default for WebSocket {
    fn default() -> WebSocket {
        WebSocket {
            handshake: None,
            base: None,
        }
    }
}

impl fmt::Display for WebSocket {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let hstr = if let Some(ref handshake) = self.handshake {
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

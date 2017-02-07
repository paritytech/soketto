use std::fmt;

#[derive(Debug, Clone)]
pub struct HandshakeFrame;

impl fmt::Display for HandshakeFrame {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "HandshakeFrame")
    }
}

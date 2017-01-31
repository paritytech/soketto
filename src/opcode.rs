/// Operation codes as part of rfc6455.
#[derive(Debug, Eq, PartialEq, Clone, Copy)]
pub enum OpCode {
    /// Indicates a continuation frame of a fragmented message.
    Continue,
    /// Indicates a text data frame.
    Text,
    /// Indicates a binary data frame.
    Binary,
    /// Indicates a close control frame.
    Close,
    /// Indicates a ping control frame.
    Ping,
    /// Indicates a pong control frame.
    Pong,
    /// Indicates a reserved op code.
    Reserved,
    /// Indicates an invalid opcode was received.
    Bad,
}

impl From<u8> for OpCode {
    fn from(val: u8) -> OpCode {
        match val {
            0 => OpCode::Continue,
            1 => OpCode::Text,
            2 => OpCode::Binary,
            8 => OpCode::Close,
            9 => OpCode::Ping,
            10 => OpCode::Pong,
            3 | 4 | 5 | 6 | 7 | 11 | 12 | 13 | 14 | 15 => OpCode::Reserved,
            _ => OpCode::Bad,
        }
    }
}

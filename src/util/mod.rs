//! General Utilities
use slog::{DrainExt, Level, level_filter, Logger};
use slog_atomic::{AtomicSwitch, AtomicSwitchCtrl};
use slog_term;
use std::io;
#[cfg(test)]
use std::io::Write;

pub mod utf8;

lazy_static! {
    /// stdout Drain switch
    pub static ref STDOUT_SW: AtomicSwitchCtrl<io::Error> = AtomicSwitch::new(
        level_filter(Level::Info, slog_term::streamer().async().compact().build())
    ).ctrl();

    /// stdout
    pub static ref STDOUT: Logger = Logger::root(STDOUT_SW.drain().fuse(), o!());

    /// stdout Drain switch
    pub static ref STDERR_SW: AtomicSwitchCtrl<io::Error> = AtomicSwitch::new(
        level_filter(Level::Error, slog_term::streamer().stderr().async().compact().build())
    ).ctrl();

    /// stderr
    pub static ref STDERR: Logger = Logger::root(STDERR_SW.drain().fuse(), o!());
}

/// Set the stdout logging `Level`.
pub fn set_stdout_level(level: Level) {
    STDOUT_SW.set(level_filter(level, slog_term::streamer().async().compact().build()))
}

#[cfg(test)]
pub fn stdo(msg: &str) {
    writeln!(io::stdout(), "{}", msg).expect("Unable to write to stdout!");
}

/// Generate an `io::ErrorKind::Other` error with the given description.
pub fn other(desc: &str) -> io::Error {
    io::Error::new(io::ErrorKind::Other, desc)
}

/// Generate a formatted hex string from a vector of bytes.
pub fn as_hex(buf: &[u8]) -> String {
    let mut hexy = String::new();

    for (idx, byte) in buf.iter().enumerate() {
        if idx % 16 == 0 {
            hexy.push_str(&format!("{:08x}:", idx))
        }

        hexy.push_str(&format!(" 0x{:02x}", byte));

        if idx % 16 == 15 {
            hexy.push('\n');
        }
    }

    hexy
}

/// Generate the hex string header.
pub fn hex_header() -> String {
    format!("{:8} {:5}{:5}{:5}{:5}{:5}{:5}{:5}{:5}{:5}{:5}{:>5}{:>5}{:>5}{:>5}{:>5}{:>5}",
            "Address",
            0,
            1,
            2,
            3,
            4,
            5,
            6,
            7,
            8,
            9,
            "a",
            "b",
            "c",
            "d",
            "e",
            "f")
}

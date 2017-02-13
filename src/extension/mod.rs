//! websocket [extensions][ext] supported by twist.
//!
//! * Per Message Deflate - Enable via the `pmdeflate` feature
//!
//! [ext]: https://tools.ietf.org/html/rfc6455#section-9
#[cfg(feature = "pmdeflate")]
pub mod deflate;

#[cfg(any(feature = "pmdeflate"))]
pub mod pm {
    use frame::WebSocket;
    use futures::{Poll, Sink, StartSend, Stream};
    use slog::Logger;
    use std::io;

    /// The `PerMessage` struct.
    pub struct PerMessage<T> {
        /// An optional slog stdout `Logger`
        stdout: Option<Logger>,
        /// A optional slog stderr `Logger`
        stderr: Option<Logger>,
        /// The upstream protocol.
        upstream: T,
        /// Enabled extensions protocols.
        #[allow(dead_code)]
        extensions: Vec<T>,
    }

    impl<T> PerMessage<T> {
        pub fn new(upstream: T) -> PerMessage<T> {
            PerMessage {
                stdout: None,
                stderr: None,
                upstream: upstream,
                extensions: Vec::new(),
            }
        }

        /// Add a slog stdout `Logger` to this `Frame` protocol
        pub fn add_stdout(&mut self, stdout: Logger) -> &mut PerMessage<T> {
            let pm_stdout = stdout.new(o!("module" => module_path!(), "proto" => "permessage"));
            self.stdout = Some(pm_stdout);
            self
        }

        /// Add a slog stderr `Logger` to this `Frame` protocol.
        pub fn add_stderr(&mut self, stderr: Logger) -> &mut PerMessage<T> {
            let pm_stderr = stderr.new(o!("module" => module_path!(), "proto" => "permessage"));
            self.stderr = Some(pm_stderr);
            self
        }
    }

    impl<T> Stream for PerMessage<T>
        where T: Stream<Item = WebSocket, Error = io::Error>,
              T: Sink<SinkItem = WebSocket, SinkError = io::Error>
    {
        type Item = WebSocket;
        type Error = io::Error;

        fn poll(&mut self) -> Poll<Option<WebSocket>, io::Error> {
            self.upstream.poll()
        }
    }

    impl<T> Sink for PerMessage<T>
        where T: Sink<SinkItem = WebSocket, SinkError = io::Error>
    {
        type SinkItem = WebSocket;
        type SinkError = io::Error;

        fn start_send(&mut self, item: WebSocket) -> StartSend<WebSocket, io::Error> {
            self.upstream.start_send(item)
        }

        fn poll_complete(&mut self) -> Poll<(), io::Error> {
            self.upstream.poll_complete()
        }
    }
}

/// The `PerFrame` struct.
#[cfg(feature = "pf")]
pub struct PerFrame<T> {
    /// An optional slog stdout `Logger`
    stdout: Option<Logger>,
    /// A optional slog stderr `Logger`
    stderr: Option<Logger>,
    /// The upstream protocol.
    upstream: T,
    /// Downstream protocols.
    downstream: Vec<T>,
}

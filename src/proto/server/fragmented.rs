//! The `Fragmented` protocol middleware.
use bytes::BytesMut;
use encoding::{Encoding, DecoderTrap};
use encoding::all::UTF_8;
use extension::{PerFrameExtensions, PerMessageExtensions};
use frame::WebSocket;
use frame::base::{Frame, OpCode};
use futures::{Async, Poll, Sink, StartSend, Stream};
use slog::Logger;
use std::{io, str};
use util;
use uuid::Uuid;

/// The `Fragmented` struct.
pub struct Fragmented<T> {
    /// The Uuid for the protocol chain.
    uuid: Uuid,
    /// The upstream protocol.
    upstream: T,
    /// Has the fragmented message started?
    started: bool,
    /// Is the fragmented message complete?
    complete: bool,
    /// The `OpCode` from the original message.
    opcode: OpCode,
    /// A running total of the payload lengths.
    total_length: u64,
    /// The buffer used to store the fragmented data.
    buf: BytesMut,
    /// Per-message extensions
    permessage_extensions: PerMessageExtensions,
    /// Per-frame extensions
    #[allow(dead_code)]
    perframe_extensions: PerFrameExtensions,
    /// slog stdout `Logger`
    stdout: Option<Logger>,
    /// slog stderr `Logger`
    stderr: Option<Logger>,
}

impl<T> Fragmented<T> {
    /// Create a new `Fragmented` protocol middleware.
    pub fn new(upstream: T,
               uuid: Uuid,
               permessage_extensions: PerMessageExtensions,
               perframe_extensions: PerFrameExtensions)
               -> Fragmented<T> {
        Fragmented {
            uuid: uuid,
            upstream: upstream,
            started: false,
            complete: false,
            opcode: OpCode::Close,
            total_length: 0,
            buf: BytesMut::with_capacity(1024),
            permessage_extensions: permessage_extensions,
            perframe_extensions: perframe_extensions,
            stdout: None,
            stderr: None,
        }
    }

    /// Add a stdout slog `Logger` to this protocol.
    pub fn stdout(&mut self, logger: Logger) -> &mut Fragmented<T> {
        let stdout = logger.new(o!("proto" => "fragmented"));
        self.stdout = Some(stdout);
        self
    }

    /// Add a stderr slog `Logger` to this protocol.
    pub fn stderr(&mut self, logger: Logger) -> &mut Fragmented<T> {
        let stderr = logger.new(o!("proto" => "fragmented"));
        self.stderr = Some(stderr);
        self
    }

    /// Run the extension chain decode on the given `base::Frame`.
    fn ext_chain_decode(&self, frame: &mut Frame) -> Result<(), io::Error> {
        let opcode = frame.opcode();
        // Only run the chain if this is a Text/Binary finish frame.
        if frame.fin() && (opcode == OpCode::Text || opcode == OpCode::Binary) {
            let pm_lock = self.permessage_extensions.clone();
            let mut map = match pm_lock.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            let vec_pm_exts = map.entry(self.uuid).or_insert_with(Vec::new);
            for ext in vec_pm_exts.iter_mut() {
                if ext.enabled() {
                    ext.decode(frame)?;
                }
            }
        }
        Ok(())
    }
}

impl<T> Stream for Fragmented<T>
    where T: Stream<Item = WebSocket, Error = io::Error>,
          T: Sink<SinkItem = WebSocket, SinkError = io::Error>
{
    type Item = WebSocket;
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Option<WebSocket>, io::Error> {
        loop {
            match try_ready!(self.upstream.poll()) {
                Some(ref msg) if msg.is_fragment_start() => {
                    if let Some(base) = msg.base() {
                        try_trace!(self.stdout, "fragment start frame received");
                        self.opcode = base.opcode();
                        self.started = true;
                        self.total_length += base.payload_length();
                        self.buf.extend(base.application_data());
                        self.poll_complete()?;
                    } else {
                        return Err(util::other("invalid fragment start frame received"));
                    }
                }
                Some(ref msg) if msg.is_fragment() => {
                    if !self.started || self.complete {
                        return Err(util::other("invalid fragment frame received"));
                    }

                    if let Some(base) = msg.base() {
                        try_trace!(self.stdout, "fragment continuation frame received");
                        self.buf.extend(base.application_data());

                        if self.opcode == OpCode::Text && self.buf.len() < 8192 {
                            match UTF_8.decode(&self.buf, DecoderTrap::Strict) {
                                Ok(_) => {}
                                Err(e) => {
                                    try_error!(self.stderr, "{}", &e);
                                    if &e != "incomplete sequence" {
                                        return Err(util::other(&e));
                                    }
                                }
                            }
                        }
                        self.poll_complete()?;
                    } else {
                        return Err(util::other("invalid fragment frame received"));
                    }
                }
                Some(ref msg) if msg.is_fragment_complete() => {
                    if !self.started || self.complete {
                        return Err(util::other("invalid fragment complete frame received"));
                    }
                    if let Some(base) = msg.base() {
                        try_trace!(self.stdout, "fragment finish frame received");
                        self.complete = true;
                        self.total_length += base.payload_length();
                        self.buf.extend(base.application_data());
                        self.poll_complete()?;
                    } else {
                        return Err(util::other("invalid fragment complete frame received"));
                    }
                }
                Some(ref msg) if msg.is_badfragment() => {
                    if self.started && !self.complete {
                        return Err(util::other("invalid opcode for continuation fragment"));
                    }
                    return Ok(Async::Ready(Some(msg.clone())));
                }
                m => return Ok(Async::Ready(m)),
            }
        }
    }
}

impl<T> Sink for Fragmented<T>
    where T: Sink<SinkItem = WebSocket, SinkError = io::Error>
{
    type SinkItem = WebSocket;
    type SinkError = io::Error;

    fn start_send(&mut self, item: WebSocket) -> StartSend<WebSocket, io::Error> {
        self.upstream.start_send(item)
    }

    fn poll_complete(&mut self) -> Poll<(), io::Error> {
        if self.started && self.complete {
            let mut message: WebSocket = Default::default();

            // Setup the `Frame` to pass upstream.
            let mut base: Frame = Default::default();
            base.set_fin(true).set_opcode(self.opcode);
            base.set_application_data(self.buf.to_vec());
            base.set_payload_length(self.total_length);

            // Run the `Frame` through the extension decode chain.
            self.ext_chain_decode(&mut base)?;

            // Validate utf-8 here to allow pre-processing of appdata by extension chain.
            if base.opcode() == OpCode::Text && base.fin() {
                match UTF_8.decode(base.application_data(), DecoderTrap::Strict) {
                    Ok(_) => {}
                    Err(e) => return Err(util::other(&e)),
                }
            }
            message.set_base(base);

            // Send it upstream
            self.upstream.start_send(message)?;

            // Reset my state.
            self.started = false;
            self.complete = false;
            self.opcode = OpCode::Close;
            self.buf.clear();

            try_trace!(self.stdout, "fragment completed sending result upstream");
        }
        self.upstream.poll_complete()
    }
}

use futures::{future, Future};
use slog::Logger;
use std::io;
use tokio_service::Service;
use twist::frame::WebSocketFrame;
use twist::frame::base::BaseFrame;

#[derive(Clone)]
pub struct PrintStdout {
    stdout: Option<Logger>,
    stderr: Option<Logger>,
}

impl PrintStdout {
    pub fn add_stdout(&mut self, stdout: Logger) -> &mut PrintStdout {
        let ps_stdout = stdout.new(o!("module" => module_path!()));
        self.stdout = Some(ps_stdout);
        self
    }

    pub fn add_stderr(&mut self, stderr: Logger) -> &mut PrintStdout {
        let ps_stderr = stderr.new(o!("module" => module_path!()));
        self.stderr = Some(ps_stderr);
        self
    }
}

impl Default for PrintStdout {
    fn default() -> PrintStdout {
        PrintStdout {
            stdout: None,
            stderr: None,
        }
    }
}

impl Service for PrintStdout {
    type Request = WebSocketFrame;
    type Response = WebSocketFrame;
    type Error = io::Error;
    type Future = Box<Future<Item = Self::Response, Error = Self::Error>>;

    fn call(&self, req: Self::Request) -> Self::Future {
        if let Some(ref stdout) = self.stdout {
            trace!(stdout, "{}", req);
        }
        if let Some(base) = req.base() {
            if let Some(ref stdout) = self.stdout {
                trace!(stdout, "Received {:?} frame", base.opcode());
            }
        }
        let base: BaseFrame = Default::default();
        let mut ws_frame: WebSocketFrame = Default::default();
        ws_frame.set_base(base);
        let resp = ws_frame;
        future::result(Ok(resp)).boxed()
    }
}

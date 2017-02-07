use futures::{future, Future, Stream};
use slog::Logger;
use std::io;
use tokio_proto::streaming::{Body, Message};
use tokio_service::Service;
use twist::frame::WebSocketFrame;
use twist::frame::base::{BaseFrame, OpCode};

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
    type Request = Message<WebSocketFrame, Body<WebSocketFrame, io::Error>>;
    type Response = Message<WebSocketFrame, Body<WebSocketFrame, io::Error>>;
    type Error = io::Error;
    type Future = Box<Future<Item = Self::Response, Error = Self::Error>>;

    fn call(&self, req: Self::Request) -> Self::Future {
        let base: BaseFrame = Default::default();
        let mut ws_frame: WebSocketFrame = Default::default();
        ws_frame.set_base(base);
        let resp = Message::WithoutBody(ws_frame);

        match req {
            Message::WithoutBody(frame) => {
                if let Some(ref stdout) = self.stdout {
                    if let Some(base) = frame.base() {
                        trace!(stdout, "Process This!: {:?}", base.app_data());
                    }
                }
                future::result(Ok(resp)).boxed()
            }
            Message::WithBody(message, body) => {
                let app_datas = body.map(|frame| if let Some(base) = frame.base() {
                        if !base.opcode().is_control() {
                            Vec::from(base.app_data())
                        } else {
                            vec![]
                        }
                    } else {
                        vec![]
                    })
                    .collect();
                let soc = if let Some(ref stdout) = self.stdout {
                    Some(stdout.clone())
                } else {
                    None
                };
                let yoblah = app_datas.map(move |v| {
                    if let Some(base) = message.base() {
                        let mut all_app_data = Vec::from(base.app_data());
                        for app_data in v.iter() {
                            all_app_data.extend(app_data);
                        }

                        if base.opcode() == OpCode::Binary {
                            if let Some(ref stdout) = soc {
                                trace!(stdout, "Process This As Binary!: {:?}", all_app_data);
                            }
                        } else {
                            if let Some(ref stdout) = soc {
                                trace!(stdout, "Process This As Text!: {:?}", all_app_data);
                            }
                        }
                    }

                    resp
                });

                Box::new(yoblah) as Self::Future
            }
        }
    }
}

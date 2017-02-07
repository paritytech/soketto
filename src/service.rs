use frame::WebsocketFrame;
use futures::{future, Future, Stream};
use opcode::OpCode;
use slog::Logger;
use std::io;
use tokio_proto::streaming::{Body, Message};
use tokio_service::Service;

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
    type Request = Message<WebsocketFrame, Body<WebsocketFrame, io::Error>>;
    type Response = Message<WebsocketFrame, Body<WebsocketFrame, io::Error>>;
    type Error = io::Error;
    type Future = Box<Future<Item = Self::Response, Error = Self::Error>>;

    fn call(&self, req: Self::Request) -> Self::Future {
        let resp = Message::WithoutBody(Default::default());

        match req {
            Message::WithoutBody(frame) => {
                if let Some(ref stdout) = self.stdout {
                    trace!(stdout, "Process This!: {:?}", frame.app_data());
                }
                future::result(Ok(resp)).boxed()
            }
            Message::WithBody(message, body) => {
                let app_datas = body.map(|frame| Vec::from(frame.app_data())).collect();
                let soc = if let Some(ref stdout) = self.stdout {
                    Some(stdout.clone())
                } else {
                    None
                };
                let yoblah = app_datas.map(move |v| {
                    let mut all_app_data = Vec::from(message.app_data());
                    for app_data in v.iter() {
                        all_app_data.extend(app_data);
                    }

                    if message.opcode() == OpCode::Binary {
                        if let Some(ref stdout) = soc {
                            trace!(stdout, "Process This As Binary!: {:?}", all_app_data);
                        }
                    } else {
                        if let Some(ref stdout) = soc {
                            trace!(stdout, "Process This As Text!: {:?}", all_app_data);
                        }
                    }
                    resp
                });

                Box::new(yoblah) as Self::Future
            }
        }
    }
}

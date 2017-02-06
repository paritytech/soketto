use frame::WebsocketFrame;
use futures::{future, Future, Stream};
use opcode::OpCode;
use slog::Logger;
use std::io;
use tokio_proto::streaming::{Body, Message};
use tokio_service::Service;

#[derive(Clone)]
pub struct Logged<T> {
    upstream: T,
    stdout: Logger
}

impl<T> Logged<T> {
    pub fn new(upstream: T, logger: Logger) -> Logged<T> {
        Logged {
            upstream: upstream,
            stdout: logger,
        }
    }
}

impl<T> Service for Logged<T>
    where T: Service,
          T::Error: From<io::Error>,
{
    type Request = T::Request;
    type Response = T::Response;
    type Error = T::Error;
    type Future = T::Future;

    fn call(&self, req: Self::Request) -> Self::Future {
        self.upstream.call(req)
    }
}

#[derive(Clone)]
pub struct PrintStdout;

impl Service for PrintStdout {
    type Request = Message<WebsocketFrame, Body<WebsocketFrame, io::Error>>;
    type Response = Message<WebsocketFrame, Body<WebsocketFrame, io::Error>>;
    type Error = io::Error;
    type Future = Box<Future<Item = Self::Response, Error = Self::Error>>;

    fn call(&self, req: Self::Request) -> Self::Future {
        let resp = Message::WithoutBody(Default::default());

        match req {
            Message::WithoutBody(frame) => {
                println!("Process This!: {:?}", frame.app_data());
                future::result(Ok(resp)).boxed()
            }
            Message::WithBody(message, body) => {
                let app_datas = body.map(|frame| Vec::from(frame.app_data())).collect();

                let yoblah = app_datas.map(move |v| {
                    let mut all_app_data = Vec::from(message.app_data());
                    for app_data in v.iter() {
                        all_app_data.extend(app_data);
                    }

                    if message.opcode() == OpCode::Binary {
                        println!("Process This As Binary!: {:?}", all_app_data);
                    } else {
                        println!("Process This As Text!: {:?}", all_app_data);
                    }
                    resp
                });

                Box::new(yoblah) as Self::Future
            }
        }
    }
}

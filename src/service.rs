use frame::WebsocketFrame;
use futures::{future, Future, Stream};
use std::io;
use tokio_proto::streaming::{Body, Message};
use tokio_service::Service;

pub struct PrintStdout;

impl Service for PrintStdout {
    type Request = Message<WebsocketFrame, Body<WebsocketFrame, io::Error>>;
    type Response = Message<WebsocketFrame, Body<WebsocketFrame, io::Error>>;
    type Error = io::Error;
    type Future = Box<Future<Item = Self::Response, Error = Self::Error>>;

    fn call(&self, req: Self::Request) -> Self::Future {
        println!("CALL: {:?}", req);
        let resp = Message::WithoutBody(Default::default());

        match req {
            Message::WithoutBody(frame) => {
                println!("Frame: {}", frame);
                Box::new(future::done(Ok(resp)))
            }
            Message::WithBody(_, body) => {
                let resp = body.for_each(|frame| {
                        println!("Body Frame: {}", frame);
                        Ok(())
                    })
                    .map(move |_| resp);

                Box::new(resp) as Self::Future
            }
        }
    }
}

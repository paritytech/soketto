extern crate byteorder;
extern crate clap;
extern crate futures;
extern crate slog_term;
extern crate tokio_core;
extern crate tokio_proto;
extern crate tokio_service;

#[macro_use]
extern crate slog;

mod frame;
mod opcode;
mod proto;
mod service;
mod util;

use clap::{App, Arg};
use proto::WebSocketProto;
use service::PrintStdout;
use slog::{DrainExt, level_filter, Level, Logger};
use std::str::FromStr;
use std::net::{IpAddr, SocketAddr};
use tokio_proto::TcpServer;

fn main() {
    let matches = App::new("twist")
        .version(env!("CARGO_PKG_VERSION"))
        .author("Jason Ozias <jason.g.ozias@gmail.com>")
        .about("RUSTFul Server for ellmak")
        .arg(Arg::with_name("address")
            .short("a")
            .long("address")
            .help("Set the address to listen on")
            .takes_value(true))
        .arg(Arg::with_name("port")
            .short("p")
            .long("port")
            .help("Set the port to listen on")
            .takes_value(true))
        .arg(Arg::with_name("verbose")
            .short("v")
            .multiple(true)
            .help("Sets the output verbosity"))
        .get_matches();

    let mut address = "127.0.0.1";
    let mut port: u16 = 3000;

    if let Some(addr_string) = matches.value_of("address") {
        address = addr_string;
    }

    if let Some(port_string) = matches.value_of("port") {
        if let Ok(port_val) = port_string.parse::<u16>() {
            port = port_val;
        }
    }

    let level = match matches.occurrences_of("verbose") {
        0 => Level::Warning,
        1 => Level::Info,
        2 => Level::Debug,
        3 | _ => Level::Trace,
    };

    let stdout_term = slog_term::streamer().compact().build();
    let stdout_drain = level_filter(level, stdout_term).fuse();
    let stdout = Logger::root(stdout_drain, o!("version" => env!("CARGO_PKG_VERSION")));

    let stderr_term = slog_term::streamer().async().stderr().compact().build();
    let stderr_drain = level_filter(Level::Error, stderr_term).fuse();
    let stderr = Logger::root(stderr_drain, o!());

    if let Ok(addr) = IpAddr::from_str(address) {
        let socket_addr = SocketAddr::new(addr, port);
        info!(stdout,
              "Listen for websocket connections on {}",
              socket_addr);
        let server = TcpServer::new(WebSocketProto, socket_addr);

        server.serve(|| Ok(PrintStdout));
    } else {
        error!(stderr, "Unable to parse address");
    }
}

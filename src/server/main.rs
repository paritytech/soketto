extern crate clap;
extern crate env_logger;
extern crate futures;
extern crate native_tls;
extern crate slog_term;
extern crate tokio_proto;
extern crate tokio_service;
extern crate tokio_tls;
extern crate twist;


#[macro_use]
extern crate slog;

mod service;

use clap::{App, Arg};
use native_tls::{Pkcs12, TlsAcceptor};
use service::PrintStdout;
use slog::{DrainExt, Level, LevelFilter, Logger};
use std::env;
use std::fs::File;
use std::io::Read;
use std::net::{IpAddr, SocketAddr};
use std::str::FromStr;
use std::thread;
use tokio_proto::TcpServer;
use tokio_tls::proto;
use twist::proto::FrameProto;

fn main() {
    env_logger::init().unwrap();
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
        .arg(Arg::with_name("tls_port")
            .short("t")
            .long("tlsport")
            .help("Set the tls port to listen on")
            .takes_value(true))
        .arg(Arg::with_name("verbose")
            .short("v")
            .multiple(true)
            .help("Sets the output verbosity"))
        .arg(Arg::with_name("pfx_file_path")
            .short("f")
            .long("pfxpath")
            .help("Set the path to the pfx file")
            .takes_value(true))
        .get_matches();

    let mut address = "127.0.0.1";
    let mut port: u16 = 11579;
    let mut tls_port: u16 = 32276;
    let mut pfx_file_path = ".env/jasonozias.com.pfx";

    if let Some(addr_string) = matches.value_of("address") {
        address = addr_string;
    }

    if let Some(port_string) = matches.value_of("port") {
        if let Ok(port_val) = port_string.parse::<u16>() {
            port = port_val;
        }
    }

    if let Some(port_string) = matches.value_of("tls_port") {
        if let Ok(port_val) = port_string.parse::<u16>() {
            tls_port = port_val;
        }
    }

    if let Some(pfx_file_path_string) = matches.value_of("pfxpath") {
        pfx_file_path = pfx_file_path_string;
    }

    let level = match matches.occurrences_of("verbose") {
        0 => Level::Warning,
        1 => Level::Info,
        2 => Level::Debug,
        3 | _ => Level::Trace,
    };

    let stdout_term = slog_term::streamer().async().compact().build();
    let stdout_drain = LevelFilter::new(stdout_term, level).fuse();
    let stdout =
        Logger::root(stdout_drain,
                     o!("version" => env!("CARGO_PKG_VERSION"), "module" => module_path!()));

    let stderr_term = slog_term::streamer().async().stderr().compact().build();
    let stderr_drain = LevelFilter::new(stderr_term, Level::Error).fuse();
    let stderr = Logger::root(stderr_drain, o!());

    let unenc_stdout = stdout.clone();
    let unenc_stderr = stderr.clone();
    let enc_stdout = stdout.clone();
    let enc_stderr = stderr.clone();

    if let Ok(addr) = IpAddr::from_str(address) {
        let unenc_socket_addr = SocketAddr::new(addr, port);
        let unenc = thread::spawn(move || {
            info!(unenc_stdout,
                  "Listening for websocket connections on {}",
                  unenc_socket_addr);
            let ws_proto = FrameProto::new(unenc_stdout.clone(), unenc_stderr.clone());
            let server = TcpServer::new(ws_proto, unenc_socket_addr);
            let mut service: PrintStdout = Default::default();
            service.add_stdout(unenc_stdout.clone()).add_stderr(unenc_stderr.clone());
            server.serve(move || Ok(service.clone()));
        });

        let enc_socket_addr = SocketAddr::new(addr, tls_port);
        if let Ok(mut file) = File::open(pfx_file_path) {
            let mut pkcs12 = vec![];
            if let Ok(_) = file.read_to_end(&mut pkcs12) {
                if let Ok(pfx_pwd) = env::var("PFX_PWD") {
                    let enc = thread::spawn(move || if let Ok(pkcs12) =
                        Pkcs12::from_der(&pkcs12, &pfx_pwd) {
                        let acceptor = TlsAcceptor::builder(pkcs12).unwrap().build().unwrap();
                        info!(enc_stdout,
                              "Listening for secure websocket connections on {}",
                              enc_socket_addr);
                        let ws_proto = FrameProto::new(enc_stdout.clone(), enc_stderr.clone());
                        let tls_proto = proto::Server::new(ws_proto, acceptor);
                        let server = TcpServer::new(tls_proto, enc_socket_addr);
                        let mut service: PrintStdout = Default::default();
                        service.add_stdout(enc_stdout.clone()).add_stderr(enc_stderr.clone());
                        server.serve(move || Ok(service.clone()));
                    } else {
                        error!(enc_stderr, "unable to convert pfx file");
                    });
                    enc.join().expect("Failed to join child thread");
                } else {
                    error!(stderr, "PFX_PWD env variable is invalid");
                }
            } else {
                error!(stderr, "unable to read pfx file into buffer");
            }
        } else {
            error!(stderr, "unable to open pfx file");
        }
        unenc.join().expect("Failed to join child thread");
    } else {
        error!(stderr, "Unable to parse address");
    }
}

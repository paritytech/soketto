# twist
A tokio-proto pipelined [ServerProto][server-proto] and tokio-core [Codec][codec] websocket implementation.

This can be used to serve up a tokio-service [Service][service] in a tokio-proto [TcpServer][tcp-server] (See [twists] for an example).

Note: Even though this is implemented as a pipeline protocol, fragmented websocket messages are supported, so streaming from a websocket client is supported.

## Usage
Include the following in your `Cargo.toml` dependencies section

    twist = "0.1.0"

## TODO
Support extensions and subprotocol negotiation.

## Documentation
[Documentation](https://docs.rs/crate/twist/0.1.0)


## Example Usage
See the [twists] project for an example implementation of an echo server using twist.

[twists]: "https://github.com/rustyhorde/twists"
[server-proto]: "https://docs.rs/tokio-proto/0.1.0/tokio_proto/pipeline/trait.ServerProto.html"
[codec]: "https://docs.rs/tokio-core/0.1.4/tokio_core/io/trait.Codec.html"
[service]: "https://docs.rs/tokio-service/0.1.0/tokio_service/trait.Service.html"
[tcp-server]: "https://docs.rs/tokio-proto/0.1.0/tokio_proto/struct.TcpServer.html"

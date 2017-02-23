# twist
An implementation of the RFC6455 websocket protocol as a tokio-core [Codec](https://docs.rs/tokio-core/0.1.4/tokio_core/io/trait.Codec.html) and a tokio-proto pipeline [ServerProto](https://docs.rs/tokio-proto/0.1.0/tokio_proto/pipeline/trait.ServerProto.html)

This can be used to serve up a tokio-service [Service](https://docs.rs/tokio-service/0.1.0/tokio_service/trait.Service.html) in a tokio-proto [TcpServer](https://docs.rs/tokio-proto/0.1.0/tokio_proto/struct.TcpServer.html) (See [twists](https://github.com/rustyhorde/twists) for an example).

Note: Even though this is implemented as a pipeline protocol, fragmented websocket messages are supported, so streaming from a websocket client is supported.

## Usage
Include the following in your `Cargo.toml` dependencies section

    twist = "0.1.3"

## Extensions
Per Message extensions are now supported by `twist`. A per-message extension is applied when a text or binary frame is complete, or a set of fragmented text or binary frames have been fully assembled.

### Implementation
`twist` exposes three traits relevant for implementing an extension:

1. `FromHeader` - Extensions are configured based on the contents of the `Sec-WebSocket-Extensions` request header.  Implement the `init` function to configure your extension based off of this header.  You should return an `io::Error` if you are unable to support the given header parameters.  If the parameters aren't relevant to your extension, you should ensure that your `enabled` function returns false.

        fn init(&mut self, request: &str) -> Result<(), io::Error> {
            if request.contains("permessage-deflate") {
                self.enabled = true;
                let pmds: Vec<&str> =
                    request.split(',').take_while(|s| s.starts_with("permessage-deflate")).collect();
                for pmd in pmds {
                    // Check the config here.
                    // return io::Error if it is invalid.
                }
            }
            Ok(())
        }

2. `IntoResponse` - Implement the `response` function to return the `Sec-WebSocket-Extensions` response header.  If you are disabled, return `None`.
3. `PerMessage` - This is the meat of the extension.  There are three methods to implement.

    a. `reserve_rsv` - If your extension requires the use of one of the frame's rsv bits, you can attempt to reserve it.   The value you receive represents the bits that have already been reserved previously in the extension chain.  Check if your bit (or bits) are free, and bitwise or your bits with the given, and return that value.  Below is an example from `twist-deflate`.  The deflate extension requires the rsv1 bit, so RSV1 is 4 (0b100).

        if current_rsv & RSV1 == 0 {
          Ok(current_rsv | RSV1)
        } else {
          Err(other("rsv1 bit already reserved"))
        }

    b. `decode` - Implement this to apply decoding to a message frame (for example decompressing the application data).

    c. `encode` - Implement this to apply encoding to a message frame before final encoding by twist (for example compressing the application data).

See [twist-deflate](https://github.com/rustyhorde/twist-deflate) for a more complete example.

## Documentation
[Documentation](https://docs.rs/twist/)

## Example Usage
See the [twists](https://github.com/rustyhorde/twists) project for an example implementation of an echo server using twist.

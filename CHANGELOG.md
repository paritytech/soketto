# 0.3.0

Update to use and work with async/await:

- `Connection` has been split into a `Sender` and `Receiver` pair with
  async methods to send and receive data or control frames such as Pings
  or Pongs.
- `connection::into_stream` has been added to get a `futures::stream::Stream`
  from a `Receiver`.
- A `connection::Builder` has been added to setup connection parameters.
  `handshake::Client` and `handshake::Server` no longer have an
  `into_connection` method, but an `into_builder` one which returns the
  `Builder` and allows further configuration.
- `base::Data` has been moved to `data`. In addition `data::Incoming`
  supports control frame data.
- `base::Codec` no longer implements `Encoder`/`Decoder` traits but has
  inherent methods for encoding and decoding websocket frame headers.
- `base::Frame` has been removed. The `base::Codec` only deals with
  headers.
- The `handshake` module contains separate sub-modules for `client` and
  `server` handshakes. Some handshake related types have been refactored
  slightly.
- `Extension`s `decode` methods work on `&mut BytesMut` parameters
  instead of `Data`. For `encode` a new type `Storage` has been added
  which unifies different types of data, i.e. shared, unique and owned data.

# 0.2.3

- Maintenance release.

# 0.2.2

- Improved handshake header matching which is now more robust and can cope with
  repeated header names and comma separated values.

# 0.2.1

- The DEFLATE extension now allows custom maximum window bits for client and server.
- Fix handling of reserved bits in base codec.

# 0.2.0

- Change `Extension` trait and add an optional DEFLATE extension (RFC 7692).
  For now the possibility to use reserved opcodes in extensions is not enabled.
  The DEFLATE extension does not support setting of window bits other than 15
  currently.
- Limit the max. buffer size in `Connection` (see `Connection::set_max_buffer_size`).

# 0.1.0

Initial release.

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

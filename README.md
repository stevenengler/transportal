# transportal

transportal is a responsive web interface and server for the
[Transmission][transmission] BitTorrent daemon. It is designed for both desktop
and mobile browsers. transportal runs as a web server and communicates with
Transmission using RPC requests. The server uses [server-sent events][sse] to
push updates to the browser.

Many features are still unimplemented or incomplete, but it works well for
simple monitoring and torrent management.

[transmission]: https://transmissionbt.com/
[sse]: https://en.wikipedia.org/wiki/Server-sent_events

## Quick start

```bash
cargo install --git https://github.com/stevenengler/transportal.git

cat >config.toml <<EOF
[connection]
bind_address = "127.0.0.1:8080"
rpc_url_base = "http://localhost:9091"
rpc_url_path = "/transmission/rpc"
EOF

transportal config.toml
```

## Limitations

Server-sent events are used to push updates to the browser. Server-sent events
work best when using http/2, but unfortunately browsers only support http/2
over TLS connections. While transportal works when not using http/2, you will
be limited to having only a few tabs open at a time. One workaround is to use a
web proxy such as Nginx, which can add TLS to the connection with a self-signed
certificate.

## Configuration

Configuration files are specified in toml format.

### `[connection]`

#### `bind_address`

*Required*

The socket address to bind the server to. Ex: `127.0.0.1:80` or
`unix:/home/user/transportal.sock`.

#### `bind_unix_perms`

Default: 600

If binding to a unix socket, these octal permissions will be used for the
socket file. The umask is ignored. Ex: `620`.

#### `rpc_url_base`

*Required*

The URL base used to connect to Transmission's RPC server. Ex:
`http://127.0.0.1:9091`.

#### `rpc_url_path`

*Required*

The URL path used to connect to Transmission's RPC server. Ex:
`/transmission/rpc`. Must have a leading slash.

### `[security]`

#### `secure_cookie_attribute`

Default: true

Whether the `Secure` attribute is set on cookies. If true, the browser must
connect over HTTPS, localhost, or an onion service. Otherwise, authentication
won't work correctly.

### `[performance]`

#### `poll_interval_ms`

Default: 1000

The interval in milliseconds at which the server polls Transmission for each
SSE connection.

## Security

transportal is still in development, so not all security protections are
complete.

### XSS

Data from Transmission is used to populate the HTML. This data is generally
escaped by the templating engine, but there are a few places that assume data
such as timestamps and torrent hashes provided by Transmission are trustworthy.
This will be improved in the future.

### CSRF

The session cookie is configured with `SameSite: Lax`, meaning that
authenticated endpoints should be protected against CSRF attacks unless the
attack comes from the same site (which includes subdomains). The login form is
not yet protected against CSRF attacks. These will be improved in the future.

### Authentication

The provided username and password are stored in memory for the duration of the
session in order to issue RPC requests to Transmission. The client is given a
random 128-bit session cookie with the `SameSite: Lax`, `HttpOnly`, and
`Secure` (unless disabled in the configuration options) attributes.

## Technical details

The server uses [axum][axum] to process HTTP requests. Transmission RPC calls
are made using [reqwest][reqwest]. HTML responses are rendered on the server
using [askama][askama], and the webpage updates dynamically using [htmx][htmx].

[axum]: https://docs.rs/axum/latest/axum/
[reqwest]: https://docs.rs/reqwest/latest/reqwest/
[askama]: https://github.com/djc/askama/tree/main
[htmx]: https://htmx.org/

## License

This program is free software: you can redistribute it and/or modify it under
the terms of the GNU Affero General Public License as published by the Free
Software Foundation, either version 3 of the License, or (at your option) any
later version.

This program is distributed in the hope that it will be useful, but WITHOUT ANY
WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A
PARTICULAR PURPOSE. See the GNU Affero General Public License for more details.

You should have received a copy of the GNU Affero General Public License along
with this program. If not, see <https://www.gnu.org/licenses/>.

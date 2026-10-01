# Wasmtime guest

Vanilla Wasmtime 49, Pulley32, on the Nanvix user ABI. The host `build.rs`
precompiles a `wasm32-unknown-unknown` module. This guest interprets it and
forwards a small import set to Nanvix TCP.

The module loaded today is `src/user/rustls-simpleserver`, the rustls
simpleserver example built with rustls-rustcrypto. It listens on port 4443.
A host client has completed a TLS 1.3 handshake (`TLS_AES_256_GCM_SHA384`)
and received `Hello from the server`.

`ring` has no RNG on `wasm32-unknown-unknown`. The server uses
rustls-rustcrypto. `UnixTime::now` is patched in the vendored
`rustls-pki-types` because that target has no wall clock unless the `web`
feature is on. The current `getrandom` hook is a fixed counter in the
module. Nanvix `getentropy` is a Park-Miller generator seeded with `12345`
and is not what this module calls.

## Imports that work

`sock_open`, `sock_bind`, `sock_listen`, `sock_accept`, `sock_connect`,
`sock_send`, `sock_recv`, `fd_close`. IPv4 only. Addresses are a host-order
`u32` and a host-order port. The guest image must include `procd`, `memd`,
and `vfsd`, and `nanvixd` must be started with `-allow-host-networking`.

## Beachhead

Programs with source link a support crate and call that instead of
`std::net`, `std::env`, and `std::fs`. The build embeds env, args, and
read-only assets, precompiles to Pulley, and links this guest.

| Call | Used for | Source |
|---|---|---|
| `tcp_*` | HTTP, Postgres, Redis, WebSockets | Implemented. |
| `now()` | certificates, JWT expiry, logs, cache TTL | Nanvix `clock_gettime` exists. The module does not call it yet. |
| `random(buf)` | TLS ephemeral keys, session ids | Needs host `getrandom`. Guest entropy is not a CSPRNG. |
| `resolve(host)` | hostnames | `getaddrinfo` is numeric IPv4 only. Resolve in the module over TCP to `1.1.1.1:53`, plus a hosts table baked at build. |
| `env` / `args` | `PORT`, `DATABASE_URL` | Bake the launch environment into the module. |
| `asset(path)` | certs, config, static files | Bake files at build, as the PEM files are today. |
| `log(msg)` | tracing | Guest debug channel. |
| `sleep` / `poll` | timeouts, more than one connection | Nanvix `nanosleep` and `select` exist. The module blocks in `accept` and `read` today. |

`now`, `random`, and `resolve` are the next slice. `poll` is the one after
that: it is what lets one thread serve more than a single client.

Out of this slice: Tokio and other threaded runtimes (Pulley has no wasm
threads), `std::process`, SQLite and other mutable filesystems, QUIC
(UDP is in the host network daemon and not wired here), and any binary
that was not compiled for this target. A stock `wasm32-wasip1` crate has
nothing to link against.

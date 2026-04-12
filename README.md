# ngmt-transport

<p align="center"><img src="branding/svg/marks/ngmt-transport.svg" width="96" height="96" alt="NGMT Transport mark"/></p>

First-party **QUIC / WAN** transport layer for **NextGenMediaTransport (NGMT)**. This repository is **greenfield**: there is no legacy Open Media Transport code here. The crate will own QUIC session logic, congestion-aware media streaming over lossy links, and eventually **FFI** to the C++ **ngmt-core** library.

## Stack (Phase 3)

- **[Quinn](https://github.com/quinn-rs/quinn)** with **`runtime-tokio`** and **`rustls`**; **ALPN** `ngmt` on client and server; **BBR** congestion control via `quinn::congestion::BbrConfig`.
- **WAN scope:** direct QUIC with lab certificates — **not** STUN/TURN/ICE yet; see the meta-repo Phase 3 plan for assumptions.
- **[Tokio](https://tokio.rs/)** for async I/O.
- **[tracing](https://docs.rs/tracing)** + **[tracing-subscriber](https://docs.rs/tracing-subscriber)** for structured diagnostics (subscriber installation is application-owned).
- **[rcgen](https://crates.io/crates/rcgen)** for ephemeral lab certificates.

## C header for C++ (`ngmt-core`)

The build script runs **[cbindgen](https://github.com/mozilla/cbindgen)** (see `cbindgen.toml`) and writes:

`include/ngmt_transport.h`

Regenerate by running `cargo build` from this repository root. **Commit** this header so CMake projects can add `target_include_directories(... include)` without running Cargo first, or document your policy if you prefer generating only in CI.

Exported symbols include **`NgmtObjectHeader`**, **`WlanOptimization`**, **`NgmtDiscoveredService`**, **`ngmt_transport_init`** / **`ngmt_transport_shutdown`**, **`ngmt_transport_peer_close`** / **`ngmt_transport_peer_dial`** / **`ngmt_transport_peer_recv_datagram_timeout`** (single global peer for hosts like OBS), **`ngmt_transport_discover_refresh`** / **`ngmt_transport_discover_count`** / **`ngmt_transport_discover_get`** / **`ngmt_transport_discover_lookup`** (DNS-SD **`_ngmt._udp`**, aligned with Studio **`ngmt-common::discovery`** behavior), and LE helpers **`ngmt_object_header_write_le`** / **`ngmt_object_header_read_le`**. Link against the **`cdylib`** artifact when integrating with C++.

## Rust API for tools (`ngmt-studio`)

The **`app_api`** module (exported from the crate root) provides **`TransportStatsSnapshot`**, **`snapshot_stats`**, **`max_ngmt_media_fragment_body`** (path-MTU-safe media fragment size), **`send_datagram`**, **`recv_datagram_async`**, **`ConnectionIntent`**, and **`JitterRing`** for Phase 4 apps. **`TransportRuntime`** exposes **`local_addr`**, **`dial`**, and **`accept_one`** for QUIC sessions (lab certificate verification; not for production).

## Smoke binary

```bash
cargo run --bin ngmt_smoke
```

## Build

```bash
cargo build
cargo test
```

Release profile:

```bash
cargo build --release
```

## Coding style

- **`rustfmt`:** configuration in [`.rustfmt.toml`](.rustfmt.toml) (100 columns, edition 2021, aligned with NGMT C++ style).

## License

MIT — see [LICENSE](LICENSE).

## Changelog

See [CHANGELOG.md](CHANGELOG.md).

## Project context

When this repo is cloned inside the NGMT meta-repository, see [`docs/project-plan/`](../docs/project-plan/) for phase roadmap and WAN validation expectations.

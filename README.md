# ngmt-transport

First-party **QUIC / WAN** transport layer for **NextGenMediaTransport (NGMT)**. This repository is **greenfield**: there is no legacy Open Media Transport code here. The crate will own QUIC session logic, congestion-aware media streaming over lossy links, and eventually **FFI** to the C++ **ngmt-core** library.

## Stack (Phase 3)

- **[Quinn](https://github.com/quinn-rs/quinn)** with **`runtime-tokio`** and **`rustls`**; **BBR** congestion control via `quinn::congestion::BbrConfig`.
- **[Tokio](https://tokio.rs/)** for async I/O.
- **[tracing](https://docs.rs/tracing)** + **[tracing-subscriber](https://docs.rs/tracing-subscriber)** for structured diagnostics (subscriber installation is application-owned).
- **[rcgen](https://crates.io/crates/rcgen)** for ephemeral lab certificates.

## C header for C++ (`ngmt-core`)

The build script runs **[cbindgen](https://github.com/mozilla/cbindgen)** (see `cbindgen.toml`) and writes:

`include/ngmt_transport.h`

Regenerate by running `cargo build` from this repository root. **Commit** this header so CMake projects can add `target_include_directories(... include)` without running Cargo first, or document your policy if you prefer generating only in CI.

Exported symbols include **`NgmtObjectHeader`**, **`WlanOptimization`**, **`ngmt_transport_init`** / **`ngmt_transport_shutdown`**, and LE helpers **`ngmt_object_header_write_le`** / **`ngmt_object_header_read_le`**. Link against the **`cdylib`** artifact when integrating with C++.

## Rust API for tools (`ngmt-studio`)

The **`app_api`** module (exported from the crate root) provides **`TransportStatsSnapshot`**, **`snapshot_stats`**, **`send_datagram`**, **`recv_datagram_async`**, **`ConnectionIntent`**, and **`JitterRing`** for Phase 4 apps. **`TransportRuntime`** exposes **`local_addr`**, **`dial`**, and **`accept_one`** for QUIC sessions (lab certificate verification; not for production).

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

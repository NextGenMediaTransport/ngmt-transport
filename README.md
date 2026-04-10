# ngmt-transport

First-party **QUIC / WAN** transport layer for **NextGenMediaTransport (NGMT)**. This repository is **greenfield**: there is no legacy Open Media Transport code here. The crate will own QUIC session logic, congestion-aware media streaming over lossy links, and eventually **FFI** to the C++ **ngmt-core** library.

## Stack (Phase 2)

- **[Quinn](https://github.com/quinn-rs/quinn)** with explicit **`runtime-tokio`** and **`rustls`** features so TLS behavior is consistent on Linux, Windows, and macOS (pure Rust / **rustls** + **ring**, no system OpenSSL in CI).
- **[Tokio](https://tokio.rs/)** for async I/O.
- **[tracing](https://docs.rs/tracing)** + **[tracing-subscriber](https://docs.rs/tracing-subscriber)** for structured diagnostics (subscriber installation is application-owned).

## C header for C++ (`ngmt-core`)

The build script runs **[cbindgen](https://github.com/mozilla/cbindgen)** and writes:

`include/ngmt_transport.h`

Regenerate by running `cargo build` from this repository root. **Commit** this header so CMake projects can add `target_include_directories(... include)` without running Cargo first, or document your policy if you prefer generating only in CI.

Exported symbols use a stable C ABI (e.g. `ngmt_transport_abi_version`). Link against the **`cdylib`** artifact when integrating with C++.

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

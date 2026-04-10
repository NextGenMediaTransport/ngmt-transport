# Changelog

All notable changes to this project will be documented in this file.

## 2026-04-10 — Phase 3: QUIC engine, FFI, BBR, WLAN tuning

### Added

- **`engine/session`**: `TransportRuntime` with **quinn** `Endpoint`, **BBR** congestion control, datagram buffers, keep-alive driven by **`WlanOptimization`**.
- **`engine/datagram_queue`**: bounded queue for unreliable payloads.
- **`ffi`**: `NgmtObjectHeader`, `WlanOptimization`, `NgmtTransportConfig`, `ngmt_transport_init` / `shutdown`, LE read/write helpers; **cbindgen** + `cbindgen.toml`.
- **`ngmt_smoke`** binary for quick ABI / endpoint bring-up.
- Dependencies: **rcgen** (dev certs), **rustls** (explicit TLS).

## 2026-04-10 — Phase 2: Rust transport crate and CI

### Added

- Initial publish to the **NextGenMediaTransport** organization (`main`); foundational Phase 2 infrastructure commit.
- Phase 2 repository initialization: Rust library crate `ngmt-transport` (edition 2021), `cdylib` + `rlib`.
- Placeholder QUIC stack: **quinn** (`runtime-tokio`, **rustls**), **tokio**, **tracing**, **tracing-subscriber**.
- **cbindgen** + `build.rs`: generates **`include/ngmt_transport.h`** (creates `include/` on first build).
- Minimal C ABI: `ngmt_transport_abi_version` for future **ngmt-core** linkage.
- MIT **LICENSE**, **`.rustfmt.toml`**, **`.cursor/rules/documentation.mdc`** (documentation mandate).
- GitHub Actions **CI** matrix: Ubuntu, Windows, macOS — `cargo build` and `cargo test`.

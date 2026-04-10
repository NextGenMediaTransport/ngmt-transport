# Changelog

All notable changes to this project will be documented in this file.

## 2026-04-10 — Phase 2: Rust transport crate and CI

### Added

- Initial publish to the **NextGenMediaTransport** organization (`main`); foundational Phase 2 infrastructure commit.
- Phase 2 repository initialization: Rust library crate `ngmt-transport` (edition 2021), `cdylib` + `rlib`.
- Placeholder QUIC stack: **quinn** (`runtime-tokio`, **rustls**), **tokio**, **tracing**, **tracing-subscriber**.
- **cbindgen** + `build.rs`: generates **`include/ngmt_transport.h`** (creates `include/` on first build).
- Minimal C ABI: `ngmt_transport_abi_version` for future **ngmt-core** linkage.
- MIT **LICENSE**, **`.rustfmt.toml`**, **`.cursor/rules/documentation.mdc`** (documentation mandate).
- GitHub Actions **CI** matrix: Ubuntu, Windows, macOS — `cargo build` and `cargo test`.

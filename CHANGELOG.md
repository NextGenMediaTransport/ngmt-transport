# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

### Changed

- **Docs:** [README.md](README.md) C header section lists **`ngmt_transport_abi_version`**, **`ngmt_transport_set_log_fn`**, and **`ngmt_transport_try_init_tracing_forwarder`**, and points integrators at committed **`include/ngmt_transport.h`**.

### Added

- **TLS / PKI (v1.0 Security Baseline prep):** Optional operator PEM paths — **`NGMT_TLS_TRUST_ANCHOR_PEM`** for QUIC clients (standard verification against pinned anchors); **`NGMT_TLS_SERVER_CERT_PEM`** + **`NGMT_TLS_SERVER_KEY_PEM`** for listener identity instead of **rcgen**. Default remains **lab** (no client verify + ephemeral server cert). Depends on **`rustls-pemfile`**. Documented in [README.md](README.md).
- **C ABI (LAN discovery):** **`NgmtDiscoveredService`**, **`ngmt_transport_discover_refresh`**, **`ngmt_transport_discover_count`**, **`ngmt_transport_discover_get`**, **`ngmt_transport_discover_lookup`** — DNS-SD browse for **`_ngmt._udp.local.`** via **`mdns-sd`** (OBS input and other C hosts; independent of QUIC init). Internal module `discover` mirrors Studio **`ngmt-common`** event mapping without a crate cycle.
- **C ABI (peer):** **`ngmt_transport_peer_close`**, **`ngmt_transport_peer_dial`**, **`ngmt_transport_peer_recv_datagram_timeout`** — single outbound QUIC connection for C hosts (e.g. OBS); **`TransportRuntime::recv_datagram_timeout`** for blocking recv on the transport runtime thread.
- **Branding:** vendored **`branding/svg/marks/ngmt-transport.svg`** + README header.
- **C ABI / OBS integration:** **`ngmt_transport_set_log_fn`** and **`ngmt_transport_try_init_tracing_forwarder`** (`log_forward` module) — optional **`tracing` → C callback** bridge when no global subscriber exists (hosts such as **OBS** can forward into **`blog()`**). Documented in generated **`include/ngmt_transport.h`**.
- **`app_api::connection_error_trace_hint`:** Maps common [`ConnectionError`](https://docs.rs/quinn/latest/quinn/enum.ConnectionError.html) cases to a short static tag for Studio trace lines (full `Debug` still logged).
- **`app_api::max_ngmt_media_fragment_body`:** Computes a safe per-path **`max_fragment_body`** for NGMT media (32-byte object header + body) from **`Connection::max_datagram_size()`** so payloads stay under the **QUIC path MTU** until discovery raises it.

### Changed

- **`TransportRuntime::connect_to`:** Logs **wall-clock ms** and **all** resolved `lookup_host` addresses to **stderr**; attempts a QUIC handshake to **each** address in order (previous code used only the first). Helps debug Studio connects when `Mac.local` returns multiple A/AAAA records.
- **`TransportRuntime::connect_to`:** Resolves are **sorted** (loopback and IPv4 LAN before unrelated `fe80::…` link-local) and each attempt uses a **3 s** handshake timeout so dead paths fail fast instead of ~30 s each.

### Fixed

- **C ABI recv:** **`ngmt_transport_peer_recv_datagram_timeout`** no longer truncates datagrams larger than **`cap`** while still reporting success (would corrupt NGMT headers). Oversized datagrams are dropped with a **`tracing::warn`** so callers can raise **`NGMT_DATAGRAM_CAP`**.
- **Studio VMX path:** Oversized application datagrams vs initial QUIC MTU (not the negotiated 64 KiB frame limit) caused immediate **`SendDatagramError::TooLarge`** after handshake; generators now cap fragment size using **`Connection::max_datagram_size()`**.
- **QUIC client ALPN:** client `rustls` configs now set **`alpn_protocols = ["ngmt"]`** to match the server, fixing failed handshakes when dialing (integration test `tests/loopback_connect.rs`, Studio outgoing mode).

### Added

- **`cbindgen.toml`:** `cpp_compat = true` so generated `include/ngmt_transport.h` wraps declarations in `extern "C"` for C++ consumers.
- **Integration test:** `tests/loopback_connect.rs` — localhost `accept_one` + `dial` smoke.

## 2026-04-10 — Phase 4: Rust API for Studio tools

### Added

- **`app_api`**: `TransportStatsSnapshot`, `snapshot_stats`, `send_datagram`, `recv_datagram_async`, `ConnectionIntent`, `JitterRing` for overlays and jitter visualization.
- **`TransportRuntime`**: `local_addr`, `dial`, `accept_one` for QUIC client/server flows; **`connect_to` / `accept_incoming`** (async) for composing connect + I/O in a **single** `block_on` (nested `block_on` on the same Tokio runtime breaks Quinn); **`close_endpoint`** to unblock accept/connect from another thread before joining a worker (UI stop vs. `JoinHandle::join` deadlock).
- Dependency: **`bytes`** for datagram payloads.

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

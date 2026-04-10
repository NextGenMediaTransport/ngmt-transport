//! Phase 3 smoke: ABI init, optional QUIC endpoint bring-up (no full stream yet).
//!
//! Metrics placeholders: discovery time, throughput @ loss, recovery — wired when orchestration exists.

use std::time::Instant;

use ngmt_transport::{
    ngmt_transport_init, ngmt_transport_shutdown, NgmtTransportConfig, WlanOptimization,
};

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let t0 = Instant::now();
    let cfg = NgmtTransportConfig {
        bind_port: 0,
        _pad0: 0,
        peer_host: std::ptr::null(),
        peer_port: 0,
        _pad1: 0,
        wlan: WlanOptimization {
            enabled: 1,
            _pad: [0; 3],
            keep_alive_interval_ms: 20,
            jitter_buffer_depth_ms: 80,
        },
    };
    let ok = ngmt_transport_init(&cfg as *const _);
    eprintln!("ngmt_transport_init: {} ({} ms)", ok, t0.elapsed().as_millis());
    ngmt_transport_shutdown();
}

//! Rust API for first-party tools (`ngmt-studio`): QUIC stats, datagram helpers, connection roles.
//!
//! C ABI remains in [`crate::ffi`]; applications should prefer this module when built as Rust.

use bytes::Bytes;
use quinn::Connection;

/// Snapshot of [`Connection::stats`] plus RTT for overlays (Monitor dashboard).
#[derive(Debug, Clone, Copy, Default)]
pub struct TransportStatsSnapshot {
    /// Smoothed RTT in milliseconds.
    pub rtt_ms: f64,
    /// BBR congestion window (bytes, QUIC path).
    pub cwnd: u64,
    pub lost_packets: u64,
    pub lost_bytes: u64,
    pub udp_tx_bytes: u64,
    pub udp_rx_bytes: u64,
    pub congestion_events: u64,
    /// Frames carrying unreliable datagrams (observability).
    pub datagram_frames_tx: u64,
    pub datagram_frames_rx: u64,
}

/// Fill snapshot from a live QUIC connection (call from UI poll / transport thread).
pub fn snapshot_stats(conn: &Connection) -> TransportStatsSnapshot {
    let s = conn.stats();
    TransportStatsSnapshot {
        rtt_ms: conn.rtt().as_secs_f64() * 1000.0,
        cwnd: s.path.cwnd,
        lost_packets: s.path.lost_packets,
        lost_bytes: s.path.lost_bytes,
        udp_tx_bytes: s.udp_tx.bytes,
        udp_rx_bytes: s.udp_rx.bytes,
        congestion_events: s.path.congestion_events,
        datagram_frames_tx: s.frame_tx.datagram,
        datagram_frames_rx: s.frame_rx.datagram,
    }
}

/// Send one unreliable datagram (NGMT object payload or fragment).
pub fn send_datagram(conn: &Connection, payload: &[u8]) -> Result<(), quinn::SendDatagramError> {
    conn.send_datagram(Bytes::copy_from_slice(payload))
}

/// Blocking receive for tooling threads (studio uses `block_on` on the transport runtime).
pub async fn recv_datagram_async(conn: &Connection) -> Result<Bytes, quinn::ConnectionError> {
    conn.read_datagram().await
}

/// High-level connection intent for documentation and UI labels (both sides use QUIC client/server).
///
/// - **Broadcast:** discoverable listener + optional browse (mDNS).
/// - **Push:** sender dials listener (outbound from generator to fixed monitor).
/// - **Pull:** receiver dials listener on source (outbound from monitor to generator).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionIntent {
    Broadcast,
    PushCaller,
    PullCaller,
}

impl ConnectionIntent {
    pub fn description(self) -> &'static str {
        match self {
            ConnectionIntent::Broadcast => "LAN discovery / advertised listener",
            ConnectionIntent::PushCaller => "Call out to peer listener (source pushes)",
            ConnectionIntent::PullCaller => "Call peer listener (receiver pulls)",
        }
    }
}

/// Rolling jitter estimate from receive timestamps (application-side, complements QUIC RTT).
pub struct JitterRing {
    cap: usize,
    samples_ms: Vec<f64>,
    idx: usize,
    jitter_buffer_depth_ms: f32,
}

impl JitterRing {
    pub fn new(capacity: usize, jitter_buffer_depth_ms: f32) -> Self {
        Self { cap: capacity.max(4), samples_ms: Vec::new(), idx: 0, jitter_buffer_depth_ms }
    }

    /// Push inter-arrival delta in ms; returns recent mean absolute deviation as "swing" hint.
    pub fn push_interarrival_ms(&mut self, delta_ms: f64) -> f64 {
        if self.samples_ms.len() < self.cap {
            self.samples_ms.push(delta_ms);
        } else {
            self.samples_ms[self.idx % self.cap] = delta_ms;
            self.idx += 1;
        }
        let n = self.samples_ms.len();
        if n < 2 {
            return 0.0;
        }
        let mean = self.samples_ms.iter().sum::<f64>() / n as f64;
        self.samples_ms.iter().map(|x| (x - mean).abs()).sum::<f64>() / n as f64
    }

    pub fn depth_hint_ms(&self) -> f32 {
        self.jitter_buffer_depth_ms
    }
}

//! NextGenMediaTransport (NGMT) — transport layer (QUIC/WAN).
//! QUIC paths for WAN media delivery; C ABI for `ngmt-core`.
//!
//! ## MoQ alignment
//! Media **objects** are carried as QUIC **datagrams** (unreliable) with optional stream-based
//! control (future). Congestion control defaults to **BBR** via `quinn-proto` when available.

pub mod app_api;
pub mod engine;
pub mod ffi;

pub use app_api::{
    recv_datagram_async, send_datagram, snapshot_stats, ConnectionIntent, JitterRing,
    TransportStatsSnapshot,
};
pub use engine::session::TransportRuntime;
pub use ffi::{
    ngmt_object_header_read_le, ngmt_object_header_write_le, ngmt_transport_abi_version,
    ngmt_transport_init, ngmt_transport_shutdown, NgmtByteSlice, NgmtObjectHeader,
    NgmtTransportConfig, WlanOptimization,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_abi_version() {
        assert_eq!(ngmt_transport_abi_version(), 1);
    }

    #[test]
    fn object_header_roundtrip_le() {
        let h = NgmtObjectHeader {
            version: 1,
            flags: 0,
            reserved: 0,
            track_id: 0x01020304,
            group_id: 0x1122334455667788,
            object_id: 0xaabbccddeeff0011,
            fragment_index: 0,
            fragment_total: 3,
            payload_length: 1200,
        };
        let mut buf = [0u8; 32];
        unsafe {
            ngmt_object_header_write_le(&h as *const _, buf.as_mut_ptr());
            let mut out = NgmtObjectHeader {
                version: 0,
                flags: 0,
                reserved: 0,
                track_id: 0,
                group_id: 0,
                object_id: 0,
                fragment_index: 0,
                fragment_total: 0,
                payload_length: 0,
            };
            assert!(ngmt_object_header_read_le(buf.as_ptr(), &mut out as *mut _));
            assert_eq!(out.track_id, h.track_id);
            assert_eq!(out.payload_length, h.payload_length);
        }
    }
}

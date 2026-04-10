//! C ABI boundary for `ngmt-core` and host applications.
//!
//! ## MoQ alignment
//! `NgmtObjectHeader` identifies **tracks** (`track_id`) and **objects** (`group_id`, `object_id`)
//! with optional **fragmentation** for datagram-sized chunks. Multi-byte fields are **little-endian**
//! on the wire; Rust `#[repr(C)]` layout matches ARM64 and x86_64 when consumers use the provided
//! serialize helpers (do not rely on implicit padding across language boundaries for extensions).

use std::os::raw::c_char;
use std::sync::{Mutex, Once, OnceLock};

use crate::engine::session::TransportRuntime;

static RUNTIME: OnceLock<Mutex<Option<TransportRuntime>>> = OnceLock::new();
static CRYPTO: Once = Once::new();

fn ensure_rustls_ring_provider() {
    CRYPTO.call_once(|| {
        rustls::crypto::ring::default_provider()
            .install_default()
            .expect("install rustls ring CryptoProvider");
    });
}

fn runtime_cell() -> &'static Mutex<Option<TransportRuntime>> {
    RUNTIME.get_or_init(|| Mutex::new(None))
}

/// Wire header for one NGMT object or fragment (little-endian integers).
/// MoQ mapping: **track** ≈ `track_id`; **group** ≈ `group_id`; **object** ≈ `object_id` + payload.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NgmtObjectHeader {
    pub version: u8,
    pub flags: u8,
    pub reserved: u16,
    pub track_id: u32,
    pub group_id: u64,
    pub object_id: u64,
    pub fragment_index: u16,
    pub fragment_total: u16,
    pub payload_length: u32,
}

/// Borrowed byte range (not owned by Rust).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct NgmtByteSlice {
    pub ptr: *const u8,
    pub len: usize,
}

/// WLAN vs wired defaults (see `ngmt_transport_init`).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct WlanOptimization {
    /// Non-zero enables aggressive keep-alive / jitter defaults.
    pub enabled: u8,
    pub _pad: [u8; 3],
    /// Suggested QUIC keep-alive / ping interval in ms (e.g. 20 for WLAN).
    pub keep_alive_interval_ms: u32,
    /// Hint for receive jitter buffer depth in ms.
    pub jitter_buffer_depth_ms: u32,
}

impl Default for WlanOptimization {
    fn default() -> Self {
        Self { enabled: 0, _pad: [0; 3], keep_alive_interval_ms: 500, jitter_buffer_depth_ms: 80 }
    }
}

/// Initialization parameters for the QUIC transport (null pointers = defaults).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct NgmtTransportConfig {
    /// 0 = pick ephemeral (client) or default listen.
    pub bind_port: u16,
    pub _pad0: u16,
    /// UTF-8 host to connect to (null = server / listen-only mode).
    pub peer_host: *const c_char,
    pub peer_port: u16,
    pub _pad1: u16,
    pub wlan: WlanOptimization,
}

impl Default for NgmtTransportConfig {
    fn default() -> Self {
        Self {
            bind_port: 0,
            _pad0: 0,
            peer_host: std::ptr::null(),
            peer_port: 0,
            _pad1: 0,
            wlan: WlanOptimization::default(),
        }
    }
}

/// Returns the current ABI version of the transport library.
#[no_mangle]
pub extern "C" fn ngmt_transport_abi_version() -> u32 {
    1
}

/// Serialize `header` to 32 bytes **little-endian** at `out` (must be at least 32 bytes).
///
/// # Safety
/// `header` and `out` must be valid for read/write of 32 bytes respectively.
#[no_mangle]
pub unsafe extern "C" fn ngmt_object_header_write_le(
    header: *const NgmtObjectHeader,
    out: *mut u8,
) {
    if header.is_null() || out.is_null() {
        return;
    }
    let h = &*header;
    let b = std::slice::from_raw_parts_mut(out, 32);
    b[0] = h.version;
    b[1] = h.flags;
    b[2..4].copy_from_slice(&h.reserved.to_le_bytes());
    b[4..8].copy_from_slice(&h.track_id.to_le_bytes());
    b[8..16].copy_from_slice(&h.group_id.to_le_bytes());
    b[16..24].copy_from_slice(&h.object_id.to_le_bytes());
    b[24..26].copy_from_slice(&h.fragment_index.to_le_bytes());
    b[26..28].copy_from_slice(&h.fragment_total.to_le_bytes());
    b[28..32].copy_from_slice(&h.payload_length.to_le_bytes());
}

/// Parse 32 little-endian bytes into `header`.
///
/// # Safety
/// `bytes` must point to at least 32 readable bytes; `out_header` must be valid for writes.
#[no_mangle]
pub unsafe extern "C" fn ngmt_object_header_read_le(
    bytes: *const u8,
    out_header: *mut NgmtObjectHeader,
) -> bool {
    if bytes.is_null() || out_header.is_null() {
        return false;
    }
    let s = std::slice::from_raw_parts(bytes, 32);
    let h = out_header.as_mut().unwrap();
    h.version = s[0];
    h.flags = s[1];
    h.reserved = u16::from_le_bytes([s[2], s[3]]);
    h.track_id = u32::from_le_bytes([s[4], s[5], s[6], s[7]]);
    h.group_id = u64::from_le_bytes([s[8], s[9], s[10], s[11], s[12], s[13], s[14], s[15]]);
    h.object_id = u64::from_le_bytes([s[16], s[17], s[18], s[19], s[20], s[21], s[22], s[23]]);
    h.fragment_index = u16::from_le_bytes([s[24], s[25]]);
    h.fragment_total = u16::from_le_bytes([s[26], s[27]]);
    h.payload_length = u32::from_le_bytes([s[28], s[29], s[30], s[31]]);
    true
}

/// Initialize the transport runtime (tokio + quinn endpoint). Safe to call once; returns false on error.
#[allow(clippy::not_unsafe_ptr_arg_deref)] // C ABI: optional pointer; validated before read.
#[no_mangle]
pub extern "C" fn ngmt_transport_init(config: *const NgmtTransportConfig) -> bool {
    ensure_rustls_ring_provider();
    let cfg = if config.is_null() {
        NgmtTransportConfig::default()
    } else {
        // SAFETY: caller guarantees `config` points to a valid struct when non-null.
        unsafe { std::ptr::read(config) }
    };
    let mut guard = match runtime_cell().lock() {
        Ok(g) => g,
        Err(_) => return false,
    };
    if guard.is_some() {
        return true;
    }
    match TransportRuntime::new(cfg) {
        Ok(rt) => {
            *guard = Some(rt);
            true
        }
        Err(e) => {
            tracing::error!(target: "ngmt_transport_ffi", "init failed: {}", e);
            false
        }
    }
}

#[no_mangle]
pub extern "C" fn ngmt_transport_shutdown() {
    if let Ok(mut g) = runtime_cell().lock() {
        *g = None;
    }
}

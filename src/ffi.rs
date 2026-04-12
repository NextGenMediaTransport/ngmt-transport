//! C ABI boundary for `ngmt-core` and host applications.
//!
//! ## MoQ alignment
//! `NgmtObjectHeader` identifies **tracks** (`track_id`) and **objects** (`group_id`, `object_id`)
//! with optional **fragmentation** for datagram-sized chunks. Multi-byte fields are **little-endian**
//! on the wire; Rust `#[repr(C)]` layout matches ARM64 and x86_64 when consumers use the provided
//! serialize helpers (do not rely on implicit padding across language boundaries for extensions).

use std::ffi::CStr;
use std::os::raw::c_char;
use std::sync::{Mutex, Once, OnceLock};
use std::time::Duration;

use quinn::Connection;

use crate::discover;
use crate::engine::session::TransportRuntime;

static RUNTIME: OnceLock<Mutex<Option<TransportRuntime>>> = OnceLock::new();
static PEER_CONN: Mutex<Option<Connection>> = Mutex::new(None);
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

/// One resolved **`_ngmt._udp`** LAN service (UTF-8, NUL-terminated fields; excess truncated).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct NgmtDiscoveredService {
    /// Target host for QUIC dial (often `*.local.`).
    pub host: [c_char; 256],
    pub port: u16,
    pub _pad: u16,
    /// DNS-SD full name, lowercased (stable `discovery_pick` key).
    pub fullname: [c_char; 256],
    /// Instance label (first label of the full name).
    pub label: [c_char; 128],
    /// Optional TXT **`role`** (`generator`, …); empty if absent.
    pub role: [c_char; 64],
}

fn write_c_field(dst: &mut [c_char], s: &str) {
    dst.fill(0);
    let max = dst.len().saturating_sub(1);
    for (i, b) in s.as_bytes().iter().take(max).enumerate() {
        dst[i] = *b as c_char;
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

/// Close the active peer QUIC connection (if any). Safe to call before/after [`ngmt_transport_shutdown`].
#[no_mangle]
pub extern "C" fn ngmt_transport_peer_close() {
    if let Ok(mut g) = PEER_CONN.lock() {
        if let Some(c) = g.take() {
            c.close(quinn::VarInt::from_u32(0), &[]);
        }
    }
}

/// Outbound QUIC dial using the global [`TransportRuntime`] (call [`ngmt_transport_init`] first).
/// Replaces any previous peer connection. `server_name` is TLS SNI (pass NULL for `"localhost"`).
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[no_mangle]
pub extern "C" fn ngmt_transport_peer_dial(
    host: *const c_char,
    port: u16,
    server_name: *const c_char,
) -> bool {
    if host.is_null() {
        return false;
    }
    ngmt_transport_peer_close();

    let host_s = match unsafe { CStr::from_ptr(host).to_str() } {
        Ok(s) if !s.is_empty() => s.to_string(),
        _ => return false,
    };
    let sn = if server_name.is_null() {
        "localhost".to_string()
    } else {
        match unsafe { CStr::from_ptr(server_name).to_str() } {
            Ok(s) if !s.is_empty() => s.to_string(),
            _ => "localhost".to_string(),
        }
    };

    let conn = {
        let g = match runtime_cell().lock() {
            Ok(x) => x,
            Err(_) => return false,
        };
        let rt = match g.as_ref() {
            Some(t) => t,
            None => return false,
        };
        match rt.dial(&host_s, port, &sn) {
            Ok(c) => c,
            Err(e) => {
                tracing::error!(target: "ngmt_transport_ffi", "peer dial failed: {}", e);
                return false;
            }
        }
    };

    match PEER_CONN.lock() {
        Ok(mut g) => {
            *g = Some(conn);
            true
        }
        Err(_) => false,
    }
}

/// Receive one datagram into `buf` (cap bytes). Writes length to `out_len` on success.
/// Blocks up to `timeout_ms` (clamped to >= 1 ms). Returns false on timeout, no connection, or error.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[no_mangle]
pub extern "C" fn ngmt_transport_peer_recv_datagram_timeout(
    buf: *mut u8,
    cap: usize,
    out_len: *mut usize,
    timeout_ms: u32,
) -> bool {
    if buf.is_null() || out_len.is_null() || cap == 0 {
        return false;
    }
    let wait = Duration::from_millis(timeout_ms.max(1) as u64);
    let g = match runtime_cell().lock() {
        Ok(x) => x,
        Err(_) => return false,
    };
    let rt = match g.as_ref() {
        Some(t) => t,
        None => return false,
    };
    let conn = {
        let pc = match PEER_CONN.lock() {
            Ok(x) => x,
            Err(_) => return false,
        };
        match pc.as_ref() {
            Some(c) => c.clone(),
            None => return false,
        }
    };
    match rt.recv_datagram_timeout(&conn, wait) {
        Ok(bytes) => {
            let len = bytes.len();
            if len > cap {
                tracing::warn!(
                    target: "ngmt_transport_ffi",
                    "peer recv datagram {} bytes > cap {} — drop (increase receive buffer)",
                    len,
                    cap
                );
                return false;
            }
            unsafe {
                std::ptr::copy_nonoverlapping(bytes.as_ptr(), buf, len);
                *out_len = len;
            }
            true
        }
        Err(e) => {
            tracing::debug!(target: "ngmt_transport_ffi", "peer recv: {}", e);
            false
        }
    }
}

#[no_mangle]
pub extern "C" fn ngmt_transport_shutdown() {
    ngmt_transport_peer_close();
    if let Ok(mut g) = runtime_cell().lock() {
        *g = None;
    }
}

/// DNS-SD browse for **`_ngmt._udp`**: collect / update the internal list by draining mDNS events for
/// roughly **`wait_ms`** (clamped 1…5000). Does **not** require [`ngmt_transport_init`]. Returns
/// `false` if the mDNS daemon failed or the browse channel died.
#[no_mangle]
pub extern "C" fn ngmt_transport_discover_refresh(wait_ms: u32) -> bool {
    let ms = (wait_ms.max(1)).min(5000);
    match discover::refresh(Duration::from_millis(ms as u64)) {
        Ok(()) => true,
        Err(e) => {
            tracing::warn!(target: "ngmt_transport_ffi", "discover_refresh: {}", e);
            false
        }
    }
}

/// Number of resolved services after the last successful [`ngmt_transport_discover_refresh`] (or 0).
#[no_mangle]
pub extern "C" fn ngmt_transport_discover_count() -> u32 {
    discover::sorted_snapshot().len() as u32
}

/// Copy service **`index`** (0 … count-1, stable sort by `fullname`) into **`out`**.
///
/// # Safety
/// `out` must be valid for writes when non-null.
#[no_mangle]
pub unsafe extern "C" fn ngmt_transport_discover_get(
    index: u32,
    out: *mut NgmtDiscoveredService,
) -> bool {
    if out.is_null() {
        return false;
    }
    let list = discover::sorted_snapshot();
    let Some(entry) = list.get(index as usize) else {
        return false;
    };
    let o = &mut *out;
    write_c_field(&mut o.host, &entry.host);
    o.port = entry.port;
    o._pad = 0;
    write_c_field(&mut o.fullname, &entry.fullname);
    write_c_field(&mut o.label, &entry.instance_name);
    write_c_field(&mut o.role, &entry.role);
    true
}

/// Look up a service by **`fullname`** (case-insensitive) in the last refreshed cache without waiting
/// on the network. Refresh first if the service may have appeared recently.
///
/// # Safety
/// `fullname` must be a valid NUL-terminated UTF-8 string when non-null. `out` must be valid for writes.
#[no_mangle]
pub unsafe extern "C" fn ngmt_transport_discover_lookup(
    fullname: *const c_char,
    out: *mut NgmtDiscoveredService,
) -> bool {
    if fullname.is_null() || out.is_null() {
        return false;
    }
    let key = match CStr::from_ptr(fullname).to_str() {
        Ok(s) if !s.is_empty() => s,
        _ => return false,
    };
    let Some(entry) = discover::lookup_fullname(key) else {
        return false;
    };
    let o = &mut *out;
    write_c_field(&mut o.host, &entry.host);
    o.port = entry.port;
    o._pad = 0;
    write_c_field(&mut o.fullname, &entry.fullname);
    write_c_field(&mut o.label, &entry.instance_name);
    write_c_field(&mut o.role, &entry.role);
    true
}

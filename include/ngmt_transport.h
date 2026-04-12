#ifndef NGMT_TRANSPORT_H
#define NGMT_TRANSPORT_H

#pragma once

#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>
#include <stddef.h>

/**
 * Wire header for one NGMT object or fragment (little-endian integers).
 * MoQ mapping: **track** ≈ `track_id`; **group** ≈ `group_id`; **object** ≈ `object_id` + payload.
 */
typedef struct NgmtObjectHeader {
  uint8_t version;
  uint8_t flags;
  uint16_t reserved;
  uint32_t track_id;
  uint64_t group_id;
  uint64_t object_id;
  uint16_t fragment_index;
  uint16_t fragment_total;
  uint32_t payload_length;
} NgmtObjectHeader;

/**
 * WLAN vs wired defaults (see `ngmt_transport_init`).
 */
typedef struct WlanOptimization {
  /**
   * Non-zero enables aggressive keep-alive / jitter defaults.
   */
  uint8_t enabled;
  uint8_t _pad[3];
  /**
   * Suggested QUIC keep-alive / ping interval in ms (e.g. 20 for WLAN).
   */
  uint32_t keep_alive_interval_ms;
  /**
   * Hint for receive jitter buffer depth in ms.
   */
  uint32_t jitter_buffer_depth_ms;
} WlanOptimization;

/**
 * Initialization parameters for the QUIC transport (null pointers = defaults).
 */
typedef struct NgmtTransportConfig {
  /**
   * 0 = pick ephemeral (client) or default listen.
   */
  uint16_t bind_port;
  uint16_t _pad0;
  /**
   * UTF-8 host to connect to (null = server / listen-only mode).
   */
  const char *peer_host;
  uint16_t peer_port;
  uint16_t _pad1;
  struct WlanOptimization wlan;
} NgmtTransportConfig;

/**
 * One resolved **`_ngmt._udp`** LAN service (UTF-8, NUL-terminated fields; excess truncated).
 */
typedef struct NgmtDiscoveredService {
  /**
   * Target host for QUIC dial (often `*.local.`).
   */
  char host[256];
  uint16_t port;
  uint16_t _pad;
  /**
   * DNS-SD full name, lowercased (stable `discovery_pick` key).
   */
  char fullname[256];
  /**
   * Instance label (first label of the full name).
   */
  char label[128];
  /**
   * Optional TXT **`role`** (`generator`, …); empty if absent.
   */
  char role[64];
} NgmtDiscoveredService;

/**
 * Borrowed byte range (not owned by Rust).
 */
typedef struct NgmtByteSlice {
  const uint8_t *ptr;
  uintptr_t len;
} NgmtByteSlice;

#ifdef __cplusplus
extern "C" {
#endif // __cplusplus

/**
 * Returns the current ABI version of the transport library.
 */
uint32_t ngmt_transport_abi_version(void);

/**
 * Serialize `header` to 32 bytes **little-endian** at `out` (must be at least 32 bytes).
 *
 * # Safety
 * `header` and `out` must be valid for read/write of 32 bytes respectively.
 */
void ngmt_object_header_write_le(const struct NgmtObjectHeader *header, uint8_t *out);

/**
 * Parse 32 little-endian bytes into `header`.
 *
 * # Safety
 * `bytes` must point to at least 32 readable bytes; `out_header` must be valid for writes.
 */
bool ngmt_object_header_read_le(const uint8_t *bytes, struct NgmtObjectHeader *out_header);

/**
 * Initialize the transport runtime (tokio + quinn endpoint). Safe to call once; returns false on error.
 */
bool ngmt_transport_init(const struct NgmtTransportConfig *config);

/**
 * Close the active peer QUIC connection (if any). Safe to call before/after [`ngmt_transport_shutdown`].
 */
void ngmt_transport_peer_close(void);

/**
 * Outbound QUIC dial using the global [`TransportRuntime`] (call [`ngmt_transport_init`] first).
 * Replaces any previous peer connection. `server_name` is TLS SNI (pass NULL for `"localhost"`).
 */
bool ngmt_transport_peer_dial(const char *host, uint16_t port, const char *server_name);

/**
 * Receive one datagram into `buf` (cap bytes). Writes length to `out_len` on success.
 * Blocks up to `timeout_ms` (clamped to >= 1 ms). Returns false on timeout, no connection, or error.
 */
bool ngmt_transport_peer_recv_datagram_timeout(uint8_t *buf,
                                               uintptr_t cap,
                                               uintptr_t *out_len,
                                               uint32_t timeout_ms);

void ngmt_transport_shutdown(void);

/**
 * DNS-SD browse for **`_ngmt._udp`**: collect / update the internal list by draining mDNS events for
 * roughly **`wait_ms`** (clamped 1…5000). Does **not** require [`ngmt_transport_init`]. Returns
 * `false` if the mDNS daemon failed or the browse channel died.
 */
bool ngmt_transport_discover_refresh(uint32_t wait_ms);

/**
 * Number of resolved services after the last successful [`ngmt_transport_discover_refresh`] (or 0).
 */
uint32_t ngmt_transport_discover_count(void);

/**
 * Copy service **`index`** (0 … count-1, stable sort by `fullname`) into **`out`**.
 *
 * # Safety
 * `out` must be valid for writes when non-null.
 */
bool ngmt_transport_discover_get(uint32_t index, struct NgmtDiscoveredService *out);

/**
 * Look up a service by **`fullname`** (case-insensitive) in the last refreshed cache without waiting
 * on the network. Refresh first if the service may have appeared recently.
 *
 * # Safety
 * `fullname` must be a valid NUL-terminated UTF-8 string when non-null. `out` must be valid for writes.
 */
bool ngmt_transport_discover_lookup(const char *fullname,
                                    struct NgmtDiscoveredService *out);

/**
 * Host sets this before `ngmt_transport_try_init_tracing_forwarder` so early events forward correctly.
 *
 * Pass `NULL` / `None` to clear the hook (e.g. on module unload).
 */
void ngmt_transport_set_log_fn(void (*cb)(int32_t, const char*));

/**
 * Install a minimal global `tracing` subscriber that forwards events to the C callback set by
 * [`ngmt_transport_set_log_fn`]. Returns `true` if this call installed the subscriber, `false` if a
 * global subscriber was already present (common when embedded with `ngmt-studio`).
 */
bool ngmt_transport_try_init_tracing_forwarder(void);

#ifdef __cplusplus
} // extern "C"
#endif // __cplusplus

#endif /* NGMT_TRANSPORT_H */

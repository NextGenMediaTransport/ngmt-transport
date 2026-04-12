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

void ngmt_transport_shutdown(void);

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

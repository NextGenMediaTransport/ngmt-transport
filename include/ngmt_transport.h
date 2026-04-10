#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

/**
 * Returns the current ABI version of the transport library.
 * Used by `ngmt-core` to verify compatibility.
 */
uint32_t ngmt_transport_abi_version(void);

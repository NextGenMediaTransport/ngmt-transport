//! NextGenMediaTransport (NGMT) — transport layer (QUIC/WAN).
//! High-performance QUIC paths for WAN media delivery; C ABI for `ngmt-core`.

/// Returns the current ABI version of the transport library.
/// Used by `ngmt-core` to verify compatibility.
#[no_mangle]
pub extern "C" fn ngmt_transport_abi_version() -> u32 {
    1
}

// Keep placeholder dependencies in the build graph until QUIC integration lands.
#[allow(dead_code)]
fn _quinn_tokio_placeholder() {
    let _ = std::mem::size_of::<quinn::Endpoint>();
    let _ = std::mem::size_of::<tokio::runtime::Runtime>();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_abi_version() {
        assert_eq!(ngmt_transport_abi_version(), 1);
    }

    #[test]
    fn tracing_macros_compile() {
        tracing::info!(target: "ngmt_transport_test", "tracing placeholder");
    }

    #[test]
    fn tracing_subscriber_env_filter_parses() {
        let _ = tracing_subscriber::EnvFilter::try_from_default_env();
    }
}

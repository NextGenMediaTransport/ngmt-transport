//! Quinn `Endpoint` wrapper: BBR congestion control, keep-alive, optional warm-up burst.
//!
//! ## MoQ alignment
//! Reliable **control** uses QUIC **streams** (future); this module establishes the **connection**
//! and enables **datagrams** for media **objects**. `WlanOptimization` tightens keep-alive and
//! jitter-buffer hints for Wi-Fi PSM / loss environments.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, UdpSocket};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use bytes::Bytes;
use quinn::congestion::BbrConfig;
use quinn::crypto::rustls::{QuicClientConfig, QuicServerConfig};
use quinn::{
    ClientConfig, Endpoint, EndpointConfig, IdleTimeout, ServerConfig, TransportConfig, VarInt,
};
use rcgen::CertifiedKey;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, ServerName, UnixTime};
use tokio::runtime::Runtime;
use tokio::time::timeout;

use crate::ffi::{NgmtTransportConfig, WlanOptimization};

fn quic_wall_ms() -> u128 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0)
}

fn quic_eprintln(msg: impl AsRef<str>) {
    eprintln!("{}", msg.as_ref());
}

/// Per-address QUIC handshake wait: failed paths (wrong interface, blackhole) fail fast instead of
/// waiting for the full idle-style timeout (~30s) on each candidate.
const CONNECT_HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(3);

/// Prefer loopback, then IPv4 LAN/private, then global unicast, then `fe80::1`, then other
/// link-local IPv6 — avoids serial 30s timeouts on irrelevant `fe80::…` from `lookup_host`.
fn sort_connect_addrs(mut addrs: Vec<SocketAddr>) -> Vec<SocketAddr> {
    fn tier(addr: &SocketAddr) -> u8 {
        match addr.ip() {
            IpAddr::V4(v4) => {
                if v4.is_loopback() {
                    0
                } else if v4.is_private() || v4.is_link_local() {
                    2
                } else {
                    4
                }
            }
            IpAddr::V6(v6) => {
                if v6.is_loopback() {
                    1
                } else if v6.segments()[0] == 0xfe80 {
                    let s = v6.segments();
                    if s[1..] == [0, 0, 0, 0, 0, 0, 1] {
                        3 // fe80::1 — often works like loopback on some stacks
                    } else {
                        6
                    }
                } else {
                    4 // global, ULA, etc. (not fe80 / not loopback)
                }
            }
        }
    }
    addrs.sort_by_key(|a| (tier(a), *a));
    addrs
}

/// Owns the tokio runtime and Quinn endpoint (Phase 3: bind + crypto + WAN tuning).
pub struct TransportRuntime {
    pub runtime: Runtime,
    pub endpoint: Endpoint,
}

fn build_transport_config(wlan: &WlanOptimization) -> TransportConfig {
    let mut t = TransportConfig::default();
    let keep_ms = if wlan.enabled != 0 {
        wlan.keep_alive_interval_ms.max(10)
    } else {
        wlan.keep_alive_interval_ms.max(250)
    };
    t.max_idle_timeout(Some(IdleTimeout::try_from(Duration::from_secs(30)).expect("idle timeout")))
        .keep_alive_interval(Some(Duration::from_millis(keep_ms as u64)))
        .datagram_send_buffer_size(4 * 1024 * 1024)
        .datagram_receive_buffer_size(Some(4 * 1024 * 1024));

    t.congestion_controller_factory(Arc::new(BbrConfig::default()));

    t
}

fn generate_certs() -> Result<(Vec<CertificateDer<'static>>, PrivateKeyDer<'static>), String> {
    let CertifiedKey { cert, key_pair } =
        rcgen::generate_simple_self_signed(vec!["localhost".into(), "ngmt.local".into()])
            .map_err(|e| e.to_string())?;
    let cert_der = cert.der().clone();
    let key = PrivateKeyDer::Pkcs8(key_pair.serialize_der().into());
    Ok((vec![cert_der], key))
}

/// Lab-only: accept any server certificate (MITM risk — never use in production).
#[derive(Debug)]
struct SkipServerVerification(Arc<rustls::crypto::CryptoProvider>);

impl SkipServerVerification {
    fn new() -> Arc<Self> {
        Arc::new(Self(Arc::new(rustls::crypto::ring::default_provider())))
    }
}

impl rustls::client::danger::ServerCertVerifier for SkipServerVerification {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp: &[u8],
        _now: UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(
            message,
            cert,
            dss,
            &self.0.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(
            message,
            cert,
            dss,
            &self.0.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        self.0.signature_verification_algorithms.supported_schemes()
    }
}

impl TransportRuntime {
    pub fn new(config: NgmtTransportConfig) -> Result<Self, String> {
        let runtime = Runtime::new().map_err(|e| e.to_string())?;

        let bind_port = if config.bind_port == 0 { 0 } else { config.bind_port };

        let socket = UdpSocket::bind(SocketAddr::from((Ipv6Addr::UNSPECIFIED, bind_port)))
            .or_else(|_| UdpSocket::bind(SocketAddr::from((Ipv4Addr::UNSPECIFIED, bind_port))))
            .map_err(|e| e.to_string())?;

        let transport = Arc::new(build_transport_config(&config.wlan));
        let is_client = !config.peer_host.is_null();

        let endpoint = {
            let _guard = runtime.enter();
            let qrt = Arc::new(quinn::TokioRuntime);

            if is_client {
                let mut client_crypto = rustls::ClientConfig::builder()
                    .dangerous()
                    .with_custom_certificate_verifier(SkipServerVerification::new())
                    .with_no_client_auth();
                // Match server ALPN (`ngmt`) so handshake succeeds (Studio dial/accept, tests).
                client_crypto.alpn_protocols = vec![b"ngmt".to_vec()];
                let mut cc = ClientConfig::new(Arc::new(
                    QuicClientConfig::try_from(client_crypto).map_err(|e| e.to_string())?,
                ));
                cc.transport_config(Arc::clone(&transport));
                Endpoint::new(EndpointConfig::default(), None, socket, qrt)
                    .map_err(|e| e.to_string())
                    .map(|mut ep| {
                        ep.set_default_client_config(cc);
                        ep
                    })?
            } else {
                let (certs, key) = generate_certs()?;
                let mut server_crypto = rustls::ServerConfig::builder()
                    .with_no_client_auth()
                    .with_single_cert(certs, key)
                    .map_err(|e| e.to_string())?;
                server_crypto.alpn_protocols = vec![b"ngmt".to_vec()];
                let mut server_config = ServerConfig::with_crypto(Arc::new(
                    QuicServerConfig::try_from(server_crypto).map_err(|e| format!("{:?}", e))?,
                ));
                server_config.transport_config(Arc::clone(&transport));
                let mut ep =
                    Endpoint::new(EndpointConfig::default(), Some(server_config), socket, qrt)
                        .map_err(|e| e.to_string())?;
                // Same endpoint can dial peers (WAN push / pull) while listening for incoming.
                let mut client_crypto = rustls::ClientConfig::builder()
                    .dangerous()
                    .with_custom_certificate_verifier(SkipServerVerification::new())
                    .with_no_client_auth();
                client_crypto.alpn_protocols = vec![b"ngmt".to_vec()];
                let mut cc = ClientConfig::new(Arc::new(
                    QuicClientConfig::try_from(client_crypto).map_err(|e| e.to_string())?,
                ));
                cc.transport_config(transport);
                ep.set_default_client_config(cc);
                ep
            }
        };

        Ok(Self { runtime, endpoint })
    }

    /// Local UDP address (after bind), for mDNS TXT and manual connect hints.
    pub fn local_addr(&self) -> Result<SocketAddr, String> {
        self.endpoint.local_addr().map_err(|e| e.to_string())
    }

    /// Async: outbound QUIC connection (client role). Prefer composing inside **one** [`Runtime::block_on`]
    /// — do not call [`Self::dial`] from inside another `block_on` on the same runtime (nested `block_on` breaks Quinn).
    pub async fn connect_to(
        &self,
        host: &str,
        port: u16,
        server_name: &str,
    ) -> Result<quinn::Connection, String> {
        let t0 = quic_wall_ms();
        quic_eprintln(format!(
            "[{t0}ms] [ngmt-transport] connect_to START host={host:?} port={port} server_name={server_name:?}"
        ));

        let addrs: Vec<SocketAddr> = tokio::net::lookup_host((host, port))
            .await
            .map_err(|e| {
                quic_eprintln(format!(
                    "[{}ms] [ngmt-transport] connect_to DNS lookup_host ERROR: {e}",
                    quic_wall_ms()
                ));
                e.to_string()
            })?
            .collect();

        if addrs.is_empty() {
            quic_eprintln(format!(
                "[{}ms] [ngmt-transport] connect_to FAIL: no addresses for {host:?}:{port}",
                quic_wall_ms()
            ));
            return Err("DNS lookup returned no addresses".to_string());
        }

        let addrs = sort_connect_addrs(addrs);

        quic_eprintln(format!(
            "[{}ms] [ngmt-transport] connect_to resolved {} addr(s) (sorted): {:?}",
            quic_wall_ms(),
            addrs.len(),
            addrs
        ));

        let mut last_err = String::from("no connection attempts");
        for (i, addr) in addrs.iter().enumerate() {
            let t = quic_wall_ms();
            quic_eprintln(format!(
                "[{t}ms] [ngmt-transport] connect_to try [{i}/{}] {addr} (handshake timeout {} ms per addr)",
                addrs.len(),
                CONNECT_HANDSHAKE_TIMEOUT.as_millis()
            ));
            let connecting = match self.endpoint.connect(*addr, server_name) {
                Ok(c) => c,
                Err(e) => {
                    last_err = e.to_string();
                    quic_eprintln(format!(
                        "[{}ms] [ngmt-transport] connect_to endpoint.connect failed for {addr}: {last_err}",
                        quic_wall_ms()
                    ));
                    continue;
                }
            };
            match timeout(CONNECT_HANDSHAKE_TIMEOUT, connecting).await {
                Ok(Ok(conn)) => {
                    quic_eprintln(format!(
                        "[{}ms] [ngmt-transport] connect_to OK via {addr} rtt={:?}",
                        quic_wall_ms(),
                        conn.stats().path.rtt
                    ));
                    return Ok(conn);
                }
                Ok(Err(e)) => {
                    last_err = e.to_string();
                    quic_eprintln(format!(
                        "[{}ms] [ngmt-transport] connect_to handshake FAILED for {addr}: {last_err}",
                        quic_wall_ms()
                    ));
                }
                Err(_) => {
                    last_err = format!(
                        "handshake timed out after {} ms",
                        CONNECT_HANDSHAKE_TIMEOUT.as_millis()
                    );
                    quic_eprintln(format!(
                        "[{}ms] [ngmt-transport] connect_to handshake TIMEOUT for {addr} ({})",
                        quic_wall_ms(),
                        last_err
                    ));
                }
            }
        }

        quic_eprintln(format!(
            "[{}ms] [ngmt-transport] connect_to EXHAUSTED all addresses. Last error: {last_err}",
            quic_wall_ms()
        ));
        Err(last_err)
    }

    /// Async: wait for the next incoming connection (server role). Same nesting rule as [`Self::connect_to`].
    pub async fn accept_incoming(&self) -> Result<quinn::Connection, String> {
        quic_eprintln(format!(
            "[{}ms] [ngmt-transport] accept_incoming waiting on endpoint.accept() (peer must dial in)",
            quic_wall_ms()
        ));
        let incoming = self.endpoint.accept().await.ok_or_else(|| {
            quic_eprintln(format!(
                "[{}ms] [ngmt-transport] accept_incoming endpoint closed (no incoming)",
                quic_wall_ms()
            ));
            "endpoint closed or not accepting".to_string()
        })?;
        quic_eprintln(format!(
            "[{}ms] [ngmt-transport] accept_incoming got Connecting; awaiting handshake",
            quic_wall_ms()
        ));
        match incoming.await {
            Ok(conn) => {
                quic_eprintln(format!(
                    "[{}ms] [ngmt-transport] accept_incoming handshake OK rtt={:?}",
                    quic_wall_ms(),
                    conn.stats().path.rtt
                ));
                Ok(conn)
            }
            Err(e) => {
                quic_eprintln(format!(
                    "[{}ms] [ngmt-transport] accept_incoming handshake ERR: {e}",
                    quic_wall_ms()
                ));
                Err(e.to_string())
            }
        }
    }

    /// Outbound QUIC connection (blocking). Safe when not already inside [`Runtime::block_on`] for this runtime.
    pub fn dial(
        &self,
        host: &str,
        port: u16,
        server_name: &str,
    ) -> Result<quinn::Connection, String> {
        self.runtime.block_on(self.connect_to(host, port, server_name))
    }

    /// Wait for the next incoming connection (blocking). Safe when not already inside `block_on` for this runtime.
    pub fn accept_one(&self) -> Result<quinn::Connection, String> {
        self.runtime.block_on(self.accept_incoming())
    }

    /// Close the QUIC endpoint (cease accepting; tear down connections). Safe to call from another thread.
    ///
    /// Use this to unblock [`accept_incoming`] / [`connect_to`] while a worker is stuck in
    /// [`Runtime::block_on`] — e.g. UI “stop” must not [`JoinHandle::join`] without closing first or the UI thread deadlocks.
    pub fn close_endpoint(&self) {
        self.endpoint.close(VarInt::from_u32(0), &[]);
    }

    /// Placeholder for post-connect bandwidth probe (call once `Connection` exists).
    pub fn warm_up_burst_ms(_duration: Duration) {
        // Future: send padding datagrams or a short unidirectional stream burst.
    }

    /// Blocking receive of one unreliable datagram (for C/FFI worker threads). Uses the same
    /// `Runtime::block_on` rule as [`Self::dial`]: do not nest `block_on` on this runtime.
    pub fn recv_datagram_timeout(
        &self,
        conn: &quinn::Connection,
        wait: Duration,
    ) -> Result<Bytes, String> {
        let c = conn.clone();
        self.runtime.block_on(async move {
            timeout(wait, c.read_datagram())
                .await
                .map_err(|_| "recv_timeout".to_string())?
                .map_err(|e| e.to_string())
        })
    }
}

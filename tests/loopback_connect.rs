//! QUIC loopback handshake: server `accept_one` + client `dial` on 127.0.0.1 (CI-friendly).

use std::ffi::CString;
use std::sync::mpsc;

use ngmt_transport::{NgmtTransportConfig, TransportRuntime, WlanOptimization};

#[test]
fn quic_loopback_accept_and_dial() {
    let (port_tx, port_rx) = mpsc::channel::<u16>();

    let server_thread = std::thread::spawn(move || {
        // Build config here: `NgmtTransportConfig` contains raw pointers and is not `Send`.
        let server_cfg = NgmtTransportConfig {
            bind_port: 0,
            _pad0: 0,
            peer_host: std::ptr::null(),
            peer_port: 0,
            _pad1: 0,
            wlan: WlanOptimization::default(),
        };
        let server = TransportRuntime::new(server_cfg).expect("server TransportRuntime");
        let port = server.local_addr().expect("server local_addr").port();
        port_tx.send(port).expect("port_tx");
        server.accept_one().expect("accept_one")
    });

    let port = port_rx.recv().expect("port_rx");

    let host = CString::new("127.0.0.1").expect("CString");
    let client_cfg = NgmtTransportConfig {
        bind_port: 0,
        _pad0: 0,
        peer_host: host.as_ptr(),
        peer_port: port,
        _pad1: 0,
        wlan: WlanOptimization::default(),
    };
    let client = TransportRuntime::new(client_cfg).expect("client TransportRuntime");
    let _client_conn = client.dial("127.0.0.1", port, "localhost").expect("dial");

    let _server_conn = server_thread.join().expect("server thread join");
}

//! DNS-SD browse for **`_ngmt._udp`** (LAN), for C hosts (OBS) without pulling in `ngmt-common`.
//!
//! Mirrors [`ngmt-common::discovery`](https://github.com/NextGenMediaTransport/NextGenMediaTransport/tree/main/ngmt-studio/crates/ngmt-common/src/discovery.rs)
//! event mapping; kept here to avoid a **`ngmt-transport` → `ngmt-common`** dependency cycle.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use flume::RecvTimeoutError;
use mdns_sd::{Receiver, ServiceDaemon, ServiceEvent};

/// Same service type string as `ngmt-common` / Studio Generator registration.
const NGMT_SERVICE_TYPE: &str = "_ngmt._udp.local.";

static DAEMON: OnceLock<Result<ServiceDaemon, String>> = OnceLock::new();

fn daemon() -> Result<&'static ServiceDaemon, String> {
    DAEMON
        .get_or_init(|| ServiceDaemon::new().map_err(|e| e.to_string()))
        .as_ref()
        .map_err(|e| e.clone())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DiscoveredEntry {
    pub fullname: String,
    pub instance_name: String,
    pub host: String,
    pub port: u16,
    pub role: String,
}

struct DiscoverState {
    browse_rx: Option<Receiver<ServiceEvent>>,
    entries: HashMap<String, DiscoveredEntry>,
}

static STATE: OnceLock<Mutex<DiscoverState>> = OnceLock::new();

fn state() -> &'static Mutex<DiscoverState> {
    STATE.get_or_init(|| Mutex::new(DiscoverState { browse_rx: None, entries: HashMap::new() }))
}

fn parse_txt_role(props: &mdns_sd::TxtProperties) -> Option<String> {
    let v = props.get_property_val_str("role")?.trim();
    if v.is_empty() {
        return None;
    }
    Some(v.to_ascii_lowercase())
}

fn apply_event(map: &mut HashMap<String, DiscoveredEntry>, ev: ServiceEvent) {
    match ev {
        ServiceEvent::ServiceResolved(info) => {
            let fullname_raw = info.get_fullname();
            let fullname = fullname_raw.to_lowercase();
            let instance_name = fullname_raw.split('.').next().unwrap_or(&fullname_raw).to_string();
            let host = info.get_hostname().trim_end_matches('.').to_string();
            let port = info.get_port();
            let role = parse_txt_role(info.get_properties()).unwrap_or_default();
            map.insert(
                fullname.clone(),
                DiscoveredEntry { fullname, instance_name, host, port, role },
            );
        }
        ServiceEvent::ServiceRemoved(_ty, fullname) => {
            map.remove(&fullname.to_lowercase());
        }
        ServiceEvent::SearchStarted(_)
        | ServiceEvent::SearchStopped(_)
        | ServiceEvent::ServiceFound(_, _) => {}
    }
}

/// Start browse if needed, then drain the browse channel until `wait` elapses (non-blocking slice recv).
pub(crate) fn refresh(wait: Duration) -> Result<(), String> {
    let d = daemon()?;
    {
        let mut s = state().lock().map_err(|_| "discover state poisoned".to_string())?;
        if s.browse_rx.is_none() {
            let rx = d.browse(NGMT_SERVICE_TYPE).map_err(|e| e.to_string())?;
            s.browse_rx = Some(rx);
        }
    }

    let deadline = Instant::now() + wait;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            break;
        }
        let slice = remaining.min(Duration::from_millis(50));
        let ev = {
            let mut s = state().lock().map_err(|_| "discover state poisoned".to_string())?;
            let rx = s.browse_rx.as_mut().ok_or_else(|| "browse receiver missing".to_string())?;
            match rx.recv_timeout(slice) {
                Ok(e) => e,
                Err(RecvTimeoutError::Timeout) => continue,
                Err(RecvTimeoutError::Disconnected) => {
                    s.browse_rx = None;
                    return Err("mDNS browse channel disconnected".to_string());
                }
            }
        };
        let mut s = state().lock().map_err(|_| "discover state poisoned".to_string())?;
        apply_event(&mut s.entries, ev);
    }
    Ok(())
}

pub(crate) fn sorted_snapshot() -> Vec<DiscoveredEntry> {
    let s = match state().lock() {
        Ok(x) => x,
        Err(_) => return Vec::new(),
    };
    let mut v: Vec<DiscoveredEntry> = s.entries.values().cloned().collect();
    v.sort_by(|a, b| a.fullname.cmp(&b.fullname));
    v
}

pub(crate) fn lookup_fullname(fullname: &str) -> Option<DiscoveredEntry> {
    let s = state().lock().ok()?;
    s.entries.get(&fullname.to_lowercase()).cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn daemon_init_idempotent() {
        let a = daemon();
        let b = daemon();
        assert!(a.is_ok() && b.is_ok());
    }
}

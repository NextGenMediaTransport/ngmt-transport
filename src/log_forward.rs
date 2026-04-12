//! Optional `tracing` → C callback bridge for hosts (e.g. OBS `blog()`) when no global subscriber exists yet.
//!
//! If another crate already installed a global `tracing` subscriber, `try_init` fails and the host
//! should rely on its own logging or install a custom layer upstream.

use std::ffi::{c_char, CString};
use std::sync::{Mutex, OnceLock};

use tracing::field::{Field, Visit};
use tracing::Subscriber;
use tracing_subscriber::layer::{Context, Layer};
use tracing_subscriber::prelude::*;
use tracing_subscriber::util::SubscriberInitExt;

static LOG_FN: Mutex<Option<unsafe extern "C" fn(i32, *const c_char)>> = Mutex::new(None);
static FORWARDER_INIT: OnceLock<bool> = OnceLock::new();

/// Host sets this before `ngmt_transport_try_init_tracing_forwarder` so early events forward correctly.
///
/// Pass `NULL` / `None` to clear the hook (e.g. on module unload).
#[no_mangle]
pub extern "C" fn ngmt_transport_set_log_fn(cb: Option<unsafe extern "C" fn(i32, *const c_char)>) {
    if let Ok(mut g) = LOG_FN.lock() {
        *g = cb;
    }
}

fn map_level(level: &tracing::Level) -> i32 {
    use tracing::Level;
    match *level {
        Level::ERROR => 400,
        Level::WARN => 300,
        Level::INFO => 250,
        Level::DEBUG => 200,
        Level::TRACE => 100,
    }
}

struct FieldBuf {
    out: String,
}

impl Default for FieldBuf {
    fn default() -> Self {
        Self {
            out: String::new(),
        }
    }
}

impl Visit for FieldBuf {
    fn record_str(&mut self, _field: &Field, value: &str) {
        if !self.out.is_empty() {
            self.out.push_str("; ");
        }
        self.out.push_str(value);
    }

    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        use std::fmt::Write;
        if !self.out.is_empty() {
            self.out.push_str("; ");
        }
        let _ = write!(&mut self.out, "{}={:?}", field.name(), value);
    }
}

struct ForwardLayer;

impl<S: Subscriber> Layer<S> for ForwardLayer {
    fn on_event(&self, event: &tracing::Event<'_>, _ctx: Context<'_, S>) {
        let cb = match LOG_FN.lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        let Some(f) = *cb else {
            return;
        };

        let mut v = FieldBuf::default();
        event.record(&mut v);
        let mut line = format!("[{}] {}", event.metadata().target(), event.metadata().name());
        if !v.out.is_empty() {
            line.push_str(" — ");
            line.push_str(&v.out);
        }

        let Ok(c) = CString::new(line) else {
            return;
        };
        let level = map_level(event.metadata().level());
        // SAFETY: callback must treat the pointer as read-only for the duration of the call.
        unsafe {
            f(level, c.as_ptr());
        }
    }
}

/// Install a minimal global `tracing` subscriber that forwards events to the C callback set by
/// [`ngmt_transport_set_log_fn`]. Returns `true` if this call installed the subscriber, `false` if a
/// global subscriber was already present (common when embedded with `ngmt-studio`).
#[no_mangle]
pub extern "C" fn ngmt_transport_try_init_tracing_forwarder() -> bool {
    *FORWARDER_INIT.get_or_init(|| {
        tracing_subscriber::registry()
            .with(ForwardLayer)
            .try_init()
            .is_ok()
    })
}

//! Bounded queue for outbound unreliable datagrams (media objects).
//!
//! ## MoQ alignment
//! Video/audio **objects** map to QUIC **datagrams**; this queue decouples capture/packetizer
//! pacing from QUIC `send_datagram` backpressure without blocking the C++ caller thread.

use std::collections::VecDeque;
use std::sync::Mutex;

/// High-priority unreliable payload waiting for `Connection::send_datagram`.
pub struct DatagramQueue {
    max_items: usize,
    inner: Mutex<VecDeque<Vec<u8>>>,
}

impl DatagramQueue {
    pub fn new(max_items: usize) -> Self {
        Self {
            max_items: max_items.max(1),
            inner: Mutex::new(VecDeque::new()),
        }
    }

    pub fn push(&self, payload: Vec<u8>) -> Result<(), Vec<u8>> {
        let mut q = self.inner.lock().map_err(|_| payload.clone())?;
        if q.len() >= self.max_items {
            return Err(payload);
        }
        q.push_back(payload);
        Ok(())
    }

    pub fn pop(&self) -> Option<Vec<u8>> {
        self.inner.lock().ok()?.pop_front()
    }

    pub fn len(&self) -> usize {
        self.inner.lock().map(|q| q.len()).unwrap_or(0)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

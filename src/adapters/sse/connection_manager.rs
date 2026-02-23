use std::sync::atomic::{AtomicBool, Ordering};

/// Manages SSE connection state.
/// Single consumer model — one connection at a time.
pub struct SseConnectionManager {
    connected: AtomicBool,
}

impl SseConnectionManager {
    pub fn new() -> Self {
        Self {
            connected: AtomicBool::new(false),
        }
    }

    pub fn is_connected(&self) -> bool {
        self.connected.load(Ordering::Relaxed)
    }

    pub fn set_connected(&self, connected: bool) {
        self.connected.store(connected, Ordering::Relaxed);
    }
}

impl Default for SseConnectionManager {
    fn default() -> Self {
        Self::new()
    }
}

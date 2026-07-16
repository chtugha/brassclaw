//! Interceptor operating mode.
//!
//! The interceptor is always running.  Its mode determines what it does
//! with each captured `ForensicPacket`:
//!
//! - **Routing** — the Sempai provider is not connected.  Packets are
//!   captured and persisted; the original Kohai prompt is forwarded
//!   unchanged.
//! - **Rerouting** — a Sempai provider is connected.  The interceptor
//!   constructs a Sempai audit prompt, sends it to the Sempai provider,
//!   receives the adjusted Kohai prompt + review data, and forwards the
//!   adjusted prompt to Kohai.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// The current mode of the interceptor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InterceptorMode {
    /// Sempai not connected: capture + forward unchanged.
    Routing,
    /// Sempai connected: capture + review + adjust before forwarding.
    Rerouting,
}

/// Shared, atomic mode flag.  The settings service flips this when the
/// operator activates or deactivates the Sempai provider via the WebUI.
///
/// Uses `SeqCst` ordering so the flag is always visible across threads
/// without relying on the caller to maintain additional synchronisation.
#[derive(Debug, Clone)]
pub struct SharedInterceptorMode(Arc<AtomicBool>);

impl SharedInterceptorMode {
    /// Create a new flag starting in **routing** mode (no Sempai connected).
    pub fn new() -> Self {
        Self(Arc::new(AtomicBool::new(false)))
    }

    /// Read the current mode.
    pub fn get(&self) -> InterceptorMode {
        if self.0.load(Ordering::SeqCst) {
            InterceptorMode::Rerouting
        } else {
            InterceptorMode::Routing
        }
    }

    /// Switch to **rerouting** mode (Sempai provider connected).
    pub fn set_rerouting(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    /// Switch back to **routing** mode (Sempai provider disconnected).
    pub fn set_routing(&self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

impl Default for SharedInterceptorMode {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_in_routing_mode() {
        let mode = SharedInterceptorMode::new();
        assert_eq!(mode.get(), InterceptorMode::Routing);
    }

    #[test]
    fn set_rerouting_switches_mode() {
        let mode = SharedInterceptorMode::new();
        mode.set_rerouting();
        assert_eq!(mode.get(), InterceptorMode::Rerouting);
    }

    #[test]
    fn set_routing_reverts_to_routing() {
        let mode = SharedInterceptorMode::new();
        mode.set_rerouting();
        mode.set_routing();
        assert_eq!(mode.get(), InterceptorMode::Routing);
    }

    #[test]
    fn clone_shares_same_flag() {
        let mode = SharedInterceptorMode::new();
        let clone = mode.clone();
        mode.set_rerouting();
        assert_eq!(clone.get(), InterceptorMode::Rerouting);
    }
}

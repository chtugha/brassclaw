//! Conversation-keyed registry of cross-turn-persistent Monty sessions (C.6
//! slice 3). Holds parked [`MontySession`] handles between turns so a single
//! Monty VM runs per conversation, parking at `host.await_next_turn()` instead
//! of returning. The registry is a pure store: it never drives a session — the
//! C.6 slice-4 `MontyTurnDriverPort` impl checks a session out, drives it
//! outside the lock, then parks or drops it.
//!
//! The core is generic over key/value so the checkout/park/drop/evict logic can
//! be unit-tested with trivial stand-ins; the production alias
//! [`MontySessionRegistry`] fixes `K = TurnScope`, `V = MontySession`.

// Transient: the registry is landed ahead of its production consumer. C.6
// slice 4 wires it into the `MontyTurnDriverPort` impl, at which point this
// allow should be removed.
#![allow(dead_code)]

use std::collections::HashMap;
use std::hash::Hash;
use std::time::{Duration, Instant};

use tokio::sync::Mutex;

use brassclaw_engine::executor::orchestrator::MontySession;
use brassclaw_turns::TurnScope;

/// A parked registry entry: the session value plus the instant it went idle.
struct Entry<V> {
    value: V,
    last_used: Instant,
}

/// Conversation-keyed store of parked sessions. The map is guarded by a
/// [`tokio::sync::Mutex`] so multiple workers can checkout/park sessions for
/// different conversations concurrently; a session is removed from the map
/// while being driven (see [`SessionRegistry::checkout_or_create`]) so the
/// lock is never held across a turn's work.
pub(crate) struct SessionRegistry<K, V> {
    entries: Mutex<HashMap<K, Entry<V>>>,
}

impl<K, V> Default for SessionRegistry<K, V>
where
    K: Hash + Eq + Clone + Send + 'static,
    V: Send + 'static,
{
    fn default() -> Self {
        Self {
            entries: Mutex::new(HashMap::new()),
        }
    }
}

impl<K, V> SessionRegistry<K, V>
where
    K: Hash + Eq + Clone + Send + 'static,
    V: Send + 'static,
{
    /// Create an empty registry.
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Remove and return the parked session for `key` if present; otherwise
    /// invoke `init` to construct a fresh one. The caller owns the returned
    /// session while driving it (the registry no longer holds it) and must
    /// either [`park`](Self::park) it (idle, awaiting the next turn) or
    /// [`drop_session`](Self::drop_session) it (the VM completed).
    ///
    /// When absent, the lock is released before `init` runs so a slow
    /// constructor does not block other conversations; same-key concurrency is
    /// prevented by the turn lease (a conversation's turns are claimed one at a
    /// time), so two checkouts for the same key cannot race.
    pub(crate) async fn checkout_or_create<E>(
        &self,
        key: &K,
        init: impl FnOnce() -> Result<V, E>,
    ) -> Result<V, E> {
        let mut entries = self.entries.lock().await;
        if let Some(entry) = entries.remove(key) {
            return Ok(entry.value);
        }
        drop(entries);
        init()
    }

    /// Park a session for `key`, stamping it idle at `Instant::now()`.
    /// Overwrites any existing entry for `key` (a prior park without an
    /// intervening checkout is stale and is replaced).
    pub(crate) async fn park(&self, key: K, value: V) {
        let mut entries = self.entries.lock().await;
        entries.insert(
            key,
            Entry {
                value,
                last_used: Instant::now(),
            },
        );
    }

    /// Remove and return the session for `key`. Use when the VM completed (the
    /// session must not be reused); returns `None` if nothing was parked.
    pub(crate) async fn drop_session(&self, key: &K) -> Option<V> {
        let mut entries = self.entries.lock().await;
        entries.remove(key).map(|entry| entry.value)
    }

    /// Evict entries idle for longer than `ttl`; returns the count evicted.
    /// Sessions being driven (checked out) are not in the map and are never
    /// evicted.
    pub(crate) async fn evict_expired(&self, ttl: Duration) -> usize {
        let mut entries = self.entries.lock().await;
        let now = Instant::now();
        let before = entries.len();
        entries.retain(|_, entry| now.duration_since(entry.last_used) <= ttl);
        before - entries.len()
    }

    /// Number of parked sessions (diagnostics/tests).
    pub(crate) async fn len(&self) -> usize {
        self.entries.lock().await.len()
    }

    /// Whether the registry holds no parked sessions.
    pub(crate) async fn is_empty(&self) -> bool {
        self.entries.lock().await.is_empty()
    }

    /// Whether a session is parked for `key` (tests).
    pub(crate) async fn contains(&self, key: &K) -> bool {
        self.entries.lock().await.contains_key(key)
    }
}

/// Production registry: one cross-turn-persistent Monty VM session per
/// conversation (`TurnScope`).
pub(crate) type MontySessionRegistry = SessionRegistry<TurnScope, MontySession>;

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    struct Key(u32);

    #[derive(Debug, Default)]
    struct MockSession {
        id: u32,
    }

    fn make_session(id: u32) -> MockSession {
        MockSession { id }
    }

    #[tokio::test]
    async fn checkout_creates_when_absent_and_leaves_map_empty() {
        let registry: SessionRegistry<Key, MockSession> = SessionRegistry::new();
        let got = registry
            .checkout_or_create(&Key(1), || Ok::<MockSession, ()>(make_session(7)))
            .await
            .unwrap();
        assert_eq!(got.id, 7);
        assert!(!registry.contains(&Key(1)).await);
        assert_eq!(registry.len().await, 0);
    }

    #[tokio::test]
    async fn park_then_checkout_returns_parked_session_without_calling_init() {
        let registry: SessionRegistry<Key, MockSession> = SessionRegistry::new();
        registry.park(Key(1), make_session(42)).await;
        assert!(registry.contains(&Key(1)).await);
        assert_eq!(registry.len().await, 1);

        let got = registry
            .checkout_or_create(&Key(1), || Err::<MockSession, ()>(()))
            .await
            .unwrap();
        assert_eq!(got.id, 42);
        assert!(!registry.contains(&Key(1)).await);
        assert_eq!(registry.len().await, 0);
    }

    #[tokio::test]
    async fn drop_session_removes_and_returns_parked() {
        let registry: SessionRegistry<Key, MockSession> = SessionRegistry::new();
        registry.park(Key(2), make_session(5)).await;
        let dropped = registry.drop_session(&Key(2)).await;
        assert_eq!(dropped.as_ref().map(|s| s.id), Some(5));
        assert!(!registry.contains(&Key(2)).await);
        assert!(registry.drop_session(&Key(2)).await.is_none());
    }

    #[tokio::test]
    async fn evict_expired_drops_idle_past_ttl() {
        let registry: SessionRegistry<Key, MockSession> = SessionRegistry::new();
        registry.park(Key(1), make_session(1)).await;
        tokio::time::sleep(Duration::from_millis(3)).await;
        let evicted = registry.evict_expired(Duration::from_millis(1)).await;
        assert_eq!(evicted, 1);
        assert!(registry.is_empty().await);
    }

    #[tokio::test]
    async fn evict_expired_keeps_fresh_entries() {
        let registry: SessionRegistry<Key, MockSession> = SessionRegistry::new();
        registry.park(Key(1), make_session(1)).await;
        let evicted = registry.evict_expired(Duration::from_secs(60)).await;
        assert_eq!(evicted, 0);
        assert!(registry.contains(&Key(1)).await);
    }

    #[tokio::test]
    async fn park_overwrites_stale_entry() {
        let registry: SessionRegistry<Key, MockSession> = SessionRegistry::new();
        registry.park(Key(1), make_session(1)).await;
        registry.park(Key(1), make_session(2)).await;
        assert_eq!(registry.len().await, 1);
        let got = registry
            .checkout_or_create(&Key(1), || Ok::<MockSession, ()>(make_session(99)))
            .await
            .unwrap();
        assert_eq!(got.id, 2);
    }

    #[test]
    fn production_alias_type_args_satisfy_registry_bounds_and_are_shareable() {
        // De-risks slice 4: the production registry can be constructed and
        // shared across workers (Arc<dyn MontyTurnDriverPort> holds it).
        fn assert_send<T: Send>() {}
        fn assert_send_sync<T: Send + Sync>() {}
        // The registry value bound is `V: Send` only. `MontySession` is `!Sync`
        // (the Monty `Heap` inside `RunProgress` is `!Sync`), but the registry
        // never shares `&MontySession` across threads — it owns the value and
        // hands it out by value under a `Mutex`.
        assert_send::<MontySession>();
        assert_send_sync::<TurnScope>();
        // `tokio::sync::Mutex<HashMap<K, Entry<V>>>` is `Send + Sync` when the
        // inner map is `Send` (`K: Send`, `V: Send`) — `MontySession: Send`
        // suffices, so the registry is shareable via `Arc`.
        assert_send_sync::<SessionRegistry<TurnScope, MontySession>>();
        assert_send_sync::<MontySessionRegistry>();
    }
}
